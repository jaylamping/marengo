//! Hardware description API: completeness + URDF lifecycle (master SoT).

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use marengo_config::{
    clear_commissioning_scope, completeness_report, default_commissioning_scope_path,
    effective_commissioning_scope, joint_subset_from_env, load_commissioning_scope,
    load_robot_config_from, merge_preview_from_paths, save_commissioning_scope, scope_widens,
    simulate_merge_xml, unresolved_critical_fields, validate_commissioning_scope_joints,
    CommissioningScopeFile, CompletenessReport, FieldResolution, MergePreview,
    COMMISSIONING_SCOPE_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::authorize_config_mutation;
use crate::restart::{now_ms, refuse_active_fresh, HEARTBEAT_FRESH_MS};
use crate::state::SharedState;

const LIVE_URDF_REL: &str = "marengo.urdf";
const CONTRIBUTOR_NAME: &str = "contributor.urdf";
const REPLACED_ACTIVE_NAME: &str = "replaced_active.urdf";

#[derive(Serialize)]
pub struct CompletenessJson {
    pub warnings: Vec<marengo_config::CompletenessWarning>,
}

#[derive(Serialize)]
pub struct UrdfUploadResultJson {
    pub ok: bool,
    pub upload_id: String,
    pub preview: MergePreview,
}

#[derive(Deserialize)]
pub struct ResolvePreviewJson {
    pub upload_id: String,
    pub resolutions: Vec<FieldResolution>,
}

#[derive(Serialize)]
pub struct ResolvePreviewResultJson {
    pub ok: bool,
    pub preview: MergePreview,
    pub unresolved_critical: Vec<String>,
    pub merged_preview_available: bool,
}

#[derive(Deserialize)]
pub struct ActivateUrdfJson {
    pub upload_id: String,
    pub resolutions: Vec<FieldResolution>,
    #[serde(default)]
    pub operator_id: String,
}

#[derive(Serialize)]
pub struct ActivateUrdfResultJson {
    pub ok: bool,
    pub message: String,
    pub checksum_sha256: String,
    pub completeness: CompletenessReport,
    pub restart_required: bool,
}

#[derive(Serialize)]
pub struct ArchiveEntryJson {
    pub upload_id: String,
    pub archived_at: Option<String>,
    pub source: Option<String>,
    pub checksum_sha256: Option<String>,
    pub contributor_checksum_sha256: Option<String>,
    pub replaced_active_checksum_sha256: Option<String>,
}

#[derive(Serialize)]
pub struct ArchiveListJson {
    pub entries: Vec<ArchiveEntryJson>,
}

#[derive(Serialize)]
pub struct ArchiveFetchJson {
    pub upload_id: String,
    pub manifest: serde_json::Value,
    pub contributor_urdf: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaced_active_urdf: Option<String>,
}

pub fn urdf_assets_root(repo_root: &Path) -> PathBuf {
    repo_root.join("assets/urdf")
}

pub fn live_urdf_path(repo_root: &Path) -> PathBuf {
    urdf_assets_root(repo_root).join(LIVE_URDF_REL)
}

pub fn staging_dir(repo_root: &Path, upload_id: &str) -> PathBuf {
    urdf_assets_root(repo_root).join("staging").join(upload_id)
}

pub fn archive_dir(repo_root: &Path, upload_id: &str) -> PathBuf {
    urdf_assets_root(repo_root).join("archive").join(upload_id)
}

fn repo_root() -> PathBuf {
    marengo_config::resolve_repo_root()
}

fn config_dir() -> PathBuf {
    marengo_config::resolve_config_dir(repo_root())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn new_upload_id() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("upload-{ms}")
}

fn validate_upload_id(id: &str) -> Result<&str, StatusCode> {
    if id.is_empty() || id.len() > 64 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return Err(StatusCode::BAD_REQUEST);
    };
    if !first.is_ascii_alphanumeric()
        || chars.any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    if Path::new(id)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(id)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StatusCode> {
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, bytes).is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    if fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(())
}

fn authorize_urdf_read(headers: &HeaderMap) -> Result<(), StatusCode> {
    authorize_config_mutation(headers)
}

pub async fn get_completeness() -> Result<Json<CompletenessJson>, StatusCode> {
    let root = repo_root();
    let dir = config_dir();
    let report = completeness_report(&root, &dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(CompletenessJson {
        warnings: report.warnings,
    }))
}

pub async fn get_urdf(headers: HeaderMap) -> Result<Response, StatusCode> {
    authorize_urdf_read(&headers)?;
    let root = repo_root();
    let path = live_urdf_path(&root);
    let bytes = fs::read(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    let checksum = sha256_hex(&bytes);
    let mut headers_out = HeaderMap::new();
    if let Ok(value) = header::HeaderValue::from_str("application/xml") {
        headers_out.insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = header::HeaderValue::from_str(&checksum) {
        headers_out.insert("x-urdf-checksum-sha256", value);
    }
    Ok((StatusCode::OK, headers_out, bytes).into_response())
}

pub async fn post_urdf_upload(
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UrdfUploadResultJson>, StatusCode> {
    authorize_config_mutation(&headers)?;
    if body.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let root = repo_root();
    let upload_id = new_upload_id();
    let staging = staging_dir(&root, &upload_id);
    fs::create_dir_all(&staging).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let contributor_path = staging.join(CONTRIBUTOR_NAME);
    fs::write(&contributor_path, &body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let master_path = live_urdf_path(&root);
    let preview = merge_preview_from_paths(&master_path, &contributor_path)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(Json(UrdfUploadResultJson {
        ok: true,
        upload_id,
        preview,
    }))
}

pub async fn post_resolve_preview(
    headers: HeaderMap,
    Json(body): Json<ResolvePreviewJson>,
) -> Result<Json<ResolvePreviewResultJson>, StatusCode> {
    authorize_config_mutation(&headers)?;
    let upload_id = validate_upload_id(&body.upload_id)?;
    let root = repo_root();
    let contributor_path = staging_dir(&root, upload_id).join(CONTRIBUTOR_NAME);
    if !contributor_path.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }
    let master_path = live_urdf_path(&root);
    let preview = merge_preview_from_paths(&master_path, &contributor_path)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let unresolved = unresolved_critical_fields(&preview, &body.resolutions);
    let unresolved_critical: Vec<String> = unresolved
        .iter()
        .map(|(joint, field)| format!("{joint}.{field}"))
        .collect();
    let merged_preview_available = unresolved.is_empty()
        && simulate_merge_from_staging(&root, upload_id, &body.resolutions).is_ok();

    Ok(Json(ResolvePreviewResultJson {
        ok: unresolved.is_empty(),
        preview,
        unresolved_critical,
        merged_preview_available,
    }))
}

pub async fn post_activate(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<ActivateUrdfJson>,
) -> Result<(StatusCode, Json<ActivateUrdfResultJson>), StatusCode> {
    authorize_config_mutation(&headers)?;
    let upload_id = validate_upload_id(&body.upload_id)?;
    let mode = state.snapshot_safety().map(|s| s.mode);
    let heartbeat_ts_ms = state.snapshot_heartbeat().map(|h| h.timestamp_ms);
    if refuse_active_fresh(mode, heartbeat_ts_ms, now_ms(), HEARTBEAT_FRESH_MS) {
        return Ok((
            StatusCode::CONFLICT,
            Json(ActivateUrdfResultJson {
                ok: false,
                message: "urdf activate refused while operational mode Active".to_string(),
                checksum_sha256: String::new(),
                completeness: CompletenessReport { warnings: vec![] },
                restart_required: false,
            }),
        ));
    }
    let root = repo_root();
    let staging = staging_dir(&root, upload_id);
    let contributor_path = staging.join(CONTRIBUTOR_NAME);
    if !contributor_path.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let master_path = live_urdf_path(&root);
    let master_xml =
        fs::read_to_string(&master_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let contributor_xml =
        fs::read_to_string(&contributor_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let preview = merge_preview_from_paths(&master_path, &contributor_path)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let unresolved = unresolved_critical_fields(&preview, &body.resolutions);
    if !unresolved.is_empty() {
        let detail = unresolved
            .iter()
            .map(|(joint, field)| format!("{joint}.{field}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(ActivateUrdfResultJson {
                ok: false,
                message: format!("kinematics-critical fields unresolved: {detail}"),
                checksum_sha256: String::new(),
                completeness: CompletenessReport { warnings: vec![] },
                restart_required: false,
            }),
        ));
    }

    let merged = simulate_merge_xml(&master_xml, &contributor_xml, &body.resolutions)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let replaced_active = master_xml.clone();
    let contributor_bytes = contributor_xml.clone();

    let archive = archive_dir(&root, upload_id);
    fs::create_dir_all(&archive).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let archive_contributor = archive.join(CONTRIBUTOR_NAME);
    let archive_replaced = archive.join(REPLACED_ACTIVE_NAME);
    write_atomic(&archive_contributor, contributor_bytes.as_bytes())?;
    write_atomic(&archive_replaced, replaced_active.as_bytes())?;
    let checksum = sha256_hex(merged.as_bytes());
    let manifest = serde_json::json!({
        "upload_id": upload_id,
        "archived_at": format_timestamp(),
        "source": "hardware/urdf/activate",
        "operator_id": if body.operator_id.is_empty() { "consul" } else { body.operator_id.as_str() },
        "contributor_urdf": CONTRIBUTOR_NAME,
        "replaced_active_urdf": REPLACED_ACTIVE_NAME,
        "checksum_sha256": checksum,
        "contributor_checksum_sha256": sha256_hex(contributor_bytes.as_bytes()),
        "replaced_active_checksum_sha256": sha256_hex(replaced_active.as_bytes()),
        "resolutions": body.resolutions,
    });
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    write_atomic(&archive.join("manifest.json"), &manifest_bytes)?;

    let tmp = master_path.with_extension("urdf.tmp");
    if fs::write(&tmp, &merged).is_err() {
        let _ = fs::remove_file(&tmp);
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ActivateUrdfResultJson {
                ok: false,
                message: "archive was saved but activate failed: could not write live URDF"
                    .to_string(),
                checksum_sha256: String::new(),
                completeness: CompletenessReport { warnings: vec![] },
                restart_required: false,
            }),
        ));
    }
    if fs::rename(&tmp, &master_path).is_err() {
        let _ = fs::remove_file(&tmp);
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ActivateUrdfResultJson {
                ok: false,
                message: "archive was saved but activate failed: could not promote live URDF"
                    .to_string(),
                checksum_sha256: String::new(),
                completeness: CompletenessReport { warnings: vec![] },
                restart_required: false,
            }),
        ));
    }

    let _ = fs::remove_dir_all(&staging);

    let completeness =
        completeness_report(&root, config_dir()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::OK,
        Json(ActivateUrdfResultJson {
            ok: true,
            message: format!("Activated merged URDF for upload {upload_id}"),
            checksum_sha256: checksum,
            completeness,
            restart_required: true,
        }),
    ))
}

pub async fn get_archive_list(headers: HeaderMap) -> Result<Json<ArchiveListJson>, StatusCode> {
    authorize_urdf_read(&headers)?;
    let root = repo_root();
    let archive_root = urdf_assets_root(&root).join("archive");
    let mut entries = Vec::new();
    if archive_root.is_dir() {
        for entry in fs::read_dir(&archive_root).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
            let entry = entry.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if !entry
                .file_type()
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .is_dir()
            {
                continue;
            }
            let upload_id = entry.file_name().to_string_lossy().to_string();
            let manifest_path = entry.path().join("manifest.json");
            let meta = if manifest_path.is_file() {
                read_manifest_summary(&manifest_path)
            } else {
                None
            };
            entries.push(ArchiveEntryJson {
                upload_id,
                archived_at: meta.as_ref().and_then(|m| m.archived_at.clone()),
                source: meta.as_ref().and_then(|m| m.source.clone()),
                checksum_sha256: meta.as_ref().and_then(|m| m.checksum_sha256.clone()),
                contributor_checksum_sha256: meta
                    .as_ref()
                    .and_then(|m| m.contributor_checksum_sha256.clone()),
                replaced_active_checksum_sha256: meta
                    .as_ref()
                    .and_then(|m| m.replaced_active_checksum_sha256.clone()),
            });
        }
    }
    entries.sort_by(|a, b| a.upload_id.cmp(&b.upload_id));
    Ok(Json(ArchiveListJson { entries }))
}

pub async fn get_archive_fetch(
    headers: HeaderMap,
    AxumPath(upload_id): AxumPath<String>,
) -> Result<Json<ArchiveFetchJson>, StatusCode> {
    authorize_urdf_read(&headers)?;
    let upload_id = validate_upload_id(&upload_id)?;
    let root = repo_root();
    let archive = archive_dir(&root, upload_id);
    if !archive.is_dir() {
        return Err(StatusCode::NOT_FOUND);
    }
    let manifest_path = archive.join("manifest.json");
    let manifest = if manifest_path.is_file() {
        let text =
            fs::read_to_string(&manifest_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };
    let contributor_path = archive.join(CONTRIBUTOR_NAME);
    let contributor_urdf =
        fs::read_to_string(&contributor_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let replaced_path = archive.join(REPLACED_ACTIVE_NAME);
    let replaced_active_urdf = if replaced_path.is_file() {
        Some(fs::read_to_string(&replaced_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
    } else {
        None
    };
    Ok(Json(ArchiveFetchJson {
        upload_id: upload_id.to_string(),
        manifest,
        contributor_urdf,
        replaced_active_urdf,
    }))
}

pub async fn post_archive_restore(
    headers: HeaderMap,
    AxumPath(upload_id): AxumPath<String>,
) -> Result<Json<UrdfUploadResultJson>, StatusCode> {
    authorize_config_mutation(&headers)?;
    let upload_id = validate_upload_id(&upload_id)?;
    let root = repo_root();
    let archive = archive_dir(&root, upload_id);
    let contributor_path = archive.join(CONTRIBUTOR_NAME);
    if !contributor_path.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }
    let staging = staging_dir(&root, upload_id);
    fs::create_dir_all(&staging).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let dest = staging.join(CONTRIBUTOR_NAME);
    fs::copy(&contributor_path, &dest).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let master_path = live_urdf_path(&root);
    let preview =
        merge_preview_from_paths(&master_path, &dest).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(UrdfUploadResultJson {
        ok: true,
        upload_id: upload_id.to_string(),
        preview,
    }))
}

fn simulate_merge_from_staging(
    repo_root: &Path,
    upload_id: &str,
    resolutions: &[FieldResolution],
) -> Result<String, marengo_config::ConfigError> {
    let master_path = live_urdf_path(repo_root);
    let contributor_path = staging_dir(repo_root, upload_id).join(CONTRIBUTOR_NAME);
    let master_xml =
        fs::read_to_string(&master_path).map_err(|error| marengo_config::ConfigError::Io {
            path: master_path,
            message: error.to_string(),
        })?;
    let contributor_xml =
        fs::read_to_string(&contributor_path).map_err(|error| marengo_config::ConfigError::Io {
            path: contributor_path,
            message: error.to_string(),
        })?;
    simulate_merge_xml(&master_xml, &contributor_xml, resolutions)
}

fn read_manifest_summary(path: &Path) -> Option<ArchiveEntryJson> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(ArchiveEntryJson {
        upload_id: value
            .get("upload_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        archived_at: value
            .get("archived_at")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        source: value
            .get("source")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        checksum_sha256: value
            .get("checksum_sha256")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        contributor_checksum_sha256: value
            .get("contributor_checksum_sha256")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        replaced_active_checksum_sha256: value
            .get("replaced_active_checksum_sha256")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

fn format_timestamp() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{ms}")
}

fn scope_path() -> PathBuf {
    default_commissioning_scope_path()
}

fn ceiling_joints() -> Option<Vec<String>> {
    joint_subset_from_env().map(|set| {
        let mut v: Vec<String> = set.into_iter().collect();
        v.sort();
        v
    })
}

#[derive(Serialize)]
pub struct CommissioningScopeResponse {
    pub version: u32,
    /// Persisted joints (empty when no scope file).
    pub joints: Vec<String>,
    /// `MARENGO_JOINT_SUBSET` ceiling when set.
    pub ceiling: Option<Vec<String>>,
    /// persisted ∩ ceiling (empty when no scope file).
    pub effective: Vec<String>,
    pub persisted: bool,
}

#[derive(Deserialize)]
pub struct PutCommissioningScopeBody {
    pub joints: Vec<String>,
    #[serde(default)]
    pub confirm_widen: bool,
}

fn scope_response(persisted: Option<&CommissioningScopeFile>) -> CommissioningScopeResponse {
    let ceiling_set = joint_subset_from_env();
    let ceiling = ceiling_joints();
    match persisted {
        Some(scope) => {
            let effective = effective_commissioning_scope(&scope.joints, ceiling_set.as_ref());
            CommissioningScopeResponse {
                version: scope.version,
                joints: scope.joints.clone(),
                ceiling,
                effective,
                persisted: true,
            }
        }
        None => CommissioningScopeResponse {
            version: COMMISSIONING_SCOPE_VERSION,
            joints: vec![],
            ceiling,
            effective: vec![],
            persisted: false,
        },
    }
}

pub async fn get_commissioning_scope(
    headers: HeaderMap,
) -> Result<Json<CommissioningScopeResponse>, StatusCode> {
    authorize_config_mutation(&headers)?;
    let loaded = load_commissioning_scope(scope_path()).map_err(|e| {
        tracing::warn!(error = %e, "commissioning scope load failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(scope_response(loaded.as_ref())))
}

pub async fn put_commissioning_scope(
    headers: HeaderMap,
    Json(body): Json<PutCommissioningScopeBody>,
) -> Result<Json<CommissioningScopeResponse>, (StatusCode, String)> {
    authorize_config_mutation(&headers).map_err(|s| (s, "unauthorized".into()))?;
    let master = load_robot_config_from(config_dir())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let next = CommissioningScopeFile::normalized(body.joints);
    validate_commissioning_scope_joints(&next.joints, &master.robot.joints)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let path = scope_path();
    let previous = load_commissioning_scope(&path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let ceiling = joint_subset_from_env();
    let prev_effective = previous
        .as_ref()
        .map(|s| effective_commissioning_scope(&s.joints, ceiling.as_ref()))
        .unwrap_or_default();
    let next_effective = effective_commissioning_scope(&next.joints, ceiling.as_ref());
    if scope_widens(&prev_effective, &next_effective) && !body.confirm_widen {
        return Err((
            StatusCode::CONFLICT,
            "confirm_widen=true required when effective scope grows".into(),
        ));
    }

    save_commissioning_scope(&path, &next)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(scope_response(Some(&next))))
}

pub async fn delete_commissioning_scope(
    headers: HeaderMap,
) -> Result<Json<CommissioningScopeResponse>, StatusCode> {
    authorize_config_mutation(&headers)?;
    clear_commissioning_scope(scope_path()).map_err(|e| {
        tracing::warn!(error = %e, "commissioning scope clear failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(scope_response(None)))
}

#[cfg(test)]
#[path = "hardware_tests.rs"]
mod tests;
