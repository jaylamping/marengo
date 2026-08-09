//! Hardware description API: completeness + URDF lifecycle (master SoT).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use marengo_config::{
    completeness_report, merge_preview_from_paths, simulate_merge_xml,
    unresolved_critical_fields, CompletenessReport, FieldResolution, MergePreview,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::authorize_config_mutation;
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
    urdf_assets_root(repo_root)
        .join("staging")
        .join(upload_id)
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
    format!("{:x}", digest)
}

fn new_upload_id() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("upload-{ms}")
}

fn authorize_urdf_read(headers: &HeaderMap) -> Result<(), StatusCode> {
    authorize_config_mutation(headers)
}

pub async fn get_completeness(
    State(state): State<SharedState>,
) -> Result<Json<CompletenessJson>, StatusCode> {
    let _logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
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
    let upload_id = body.upload_id.trim();
    if upload_id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
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
    headers: HeaderMap,
    Json(body): Json<ActivateUrdfJson>,
) -> Result<(StatusCode, Json<ActivateUrdfResultJson>), StatusCode> {
    authorize_config_mutation(&headers)?;
    let upload_id = body.upload_id.trim();
    if upload_id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let root = repo_root();
    let staging = staging_dir(&root, upload_id);
    let contributor_path = staging.join(CONTRIBUTOR_NAME);
    if !contributor_path.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let master_path = live_urdf_path(&root);
    let master_xml = fs::read_to_string(&master_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
    fs::write(&archive_contributor, contributor_bytes.as_bytes())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    fs::write(&archive_replaced, replaced_active.as_bytes())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tmp = master_path.with_extension("urdf.tmp");
    fs::write(&tmp, &merged).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Err(_) = fs::rename(&tmp, &master_path) {
        let _ = fs::remove_file(&tmp);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let _ = fs::remove_dir_all(&staging);

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
    fs::write(
        archive.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap_or_default(),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let completeness = completeness_report(&root, &config_dir())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::OK,
        Json(ActivateUrdfResultJson {
            ok: true,
            message: format!("Activated merged URDF for upload {upload_id}"),
            checksum_sha256: checksum,
            completeness,
        }),
    ))
}

pub async fn get_archive_list() -> Result<Json<ArchiveListJson>, StatusCode> {
    let root = repo_root();
    let archive_root = urdf_assets_root(&root).join("archive");
    let mut entries = Vec::new();
    if archive_root.is_dir() {
        for entry in fs::read_dir(&archive_root).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
            let entry = entry.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if !entry.file_type().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.is_dir() {
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
    AxumPath(upload_id): AxumPath<String>,
) -> Result<Json<ArchiveFetchJson>, StatusCode> {
    let root = repo_root();
    let archive = archive_dir(&root, &upload_id);
    if !archive.is_dir() {
        return Err(StatusCode::NOT_FOUND);
    }
    let manifest_path = archive.join("manifest.json");
    let manifest = if manifest_path.is_file() {
        let text = fs::read_to_string(&manifest_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };
    let contributor_path = archive.join(CONTRIBUTOR_NAME);
    let contributor_urdf = fs::read_to_string(&contributor_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let replaced_path = archive.join(REPLACED_ACTIVE_NAME);
    let replaced_active_urdf = if replaced_path.is_file() {
        Some(fs::read_to_string(&replaced_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
    } else {
        None
    };
    Ok(Json(ArchiveFetchJson {
        upload_id,
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
    let root = repo_root();
    let archive = archive_dir(&root, &upload_id);
    let contributor_path = archive.join(CONTRIBUTOR_NAME);
    if !contributor_path.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }
    let staging = staging_dir(&root, &upload_id);
    fs::create_dir_all(&staging).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let dest = staging.join(CONTRIBUTOR_NAME);
    fs::copy(&contributor_path, &dest).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let master_path = live_urdf_path(&root);
    let preview = merge_preview_from_paths(&master_path, &dest).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(UrdfUploadResultJson {
        ok: true,
        upload_id,
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
    let master_xml = fs::read_to_string(&master_path).map_err(|error| {
        marengo_config::ConfigError::Io {
            path: master_path,
            message: error.to_string(),
        }
    })?;
    let contributor_xml = fs::read_to_string(&contributor_path).map_err(|error| {
        marengo_config::ConfigError::Io {
            path: contributor_path,
            message: error.to_string(),
        }
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
        archived_at: value.get("archived_at").and_then(|v| v.as_str()).map(str::to_string),
        source: value.get("source").and_then(|v| v.as_str()).map(str::to_string),
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

#[cfg(test)]
#[path = "hardware_tests.rs"]
mod tests;
