/**
 * PROTOTYPE — vanilla Three.js humanoid joint picker.
 *
 * No react-three-fiber: the scene owns its renderer, loop, and DOM overlay, and
 * React only mounts it and pushes selection in. Rendering is dirty-flagged, so
 * an idle picker costs nothing.
 */
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

import { ANCHORS, BONES, RIG_FOCUS, type Vec3 } from './humanoid-rig';
import { jointPosition, type Limb, type ProtoJoint } from './mock-hardware';

/** Launch Day palette, sampled to hex for WebGL. */
const COLOR = {
  void: 0x14161a,
  bone: 0x3a4048,
  boneSchematic: 0x1c2027,
  edge: 0x5b636d,
  grid: 0x252b33,
  accent: 0xffb000,
  ok: 0x2fd39b,
  neutral: 0x767d87,
  key: 0xf4f6f8,
  rim: 0x8fb4d6,
} as const;

export type SceneStyle = 'solid' | 'schematic';

export type HumanoidSceneOptions = {
  container: HTMLElement;
  joints: ProtoJoint[];
  style?: SceneStyle;
  /** Orbit + perspective, or locked orthographic front elevation. */
  orbit?: boolean;
  /** Show the ground grid and shadow catcher. */
  ground?: boolean;
  onSelect: (id: string | null) => void;
  onHover?: (id: string | null) => void;
};

type JointVisual = {
  joint: ProtoJoint;
  marker: THREE.Mesh;
  hitTarget: THREE.Mesh;
  label: HTMLDivElement;
  baseColor: THREE.Color;
  position: THREE.Vector3;
};

const vec = (v: Vec3) => new THREE.Vector3(v[0], v[1], v[2]);

function statusColor(joint: ProtoJoint): number {
  if (joint.completenessWarn) return COLOR.accent;
  if (joint.onCan) return COLOR.ok;
  return COLOR.neutral;
}

export class HumanoidScene {
  private readonly container: HTMLElement;
  private readonly overlay: HTMLDivElement;
  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene = new THREE.Scene();
  private readonly camera: THREE.PerspectiveCamera | THREE.OrthographicCamera;
  private readonly controls: OrbitControls | null = null;
  private readonly raycaster = new THREE.Raycaster();
  private readonly pointer = new THREE.Vector2();
  private readonly resizeObserver: ResizeObserver;

  private readonly visuals = new Map<string, JointVisual>();
  private readonly hitTargets: THREE.Mesh[] = [];
  private readonly boneMeshes: { mesh: THREE.Object3D; limb: Limb }[] = [];
  private readonly selectionRing: THREE.Mesh;
  private readonly disposables: { dispose: () => void }[] = [];

  private readonly onSelect: (id: string | null) => void;
  private readonly onHover: ((id: string | null) => void) | undefined;
  private readonly orthographic: boolean;

  private selectedId: string | null = null;
  private hoveredId: string | null = null;
  private focusLimb: Limb | null = null;
  private lastHoverTest = 0;
  private frame = 0;
  private dirty = true;
  private disposed = false;

  constructor(options: HumanoidSceneOptions) {
    const { container, joints, style = 'solid', orbit = true, ground = true } = options;

    this.container = container;
    this.onSelect = options.onSelect;
    this.onHover = options.onHover;
    this.orthographic = !orbit;

    const { clientWidth: width, clientHeight: height } = container;

    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
      powerPreference: 'high-performance',
    });
    this.renderer.setSize(width, height, false);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.05;
    this.renderer.shadowMap.enabled = ground;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.renderer.setClearColor(COLOR.void, style === 'schematic' ? 1 : 0.92);
    this.renderer.domElement.style.display = 'block';
    this.renderer.domElement.style.touchAction = 'none';
    container.appendChild(this.renderer.domElement);

    this.overlay = document.createElement('div');
    this.overlay.style.cssText =
      'position:absolute;inset:0;pointer-events:none;overflow:hidden;';
    container.appendChild(this.overlay);

    if (this.orthographic) {
      this.camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.01, 100);
      this.camera.position.set(0, RIG_FOCUS[1], 6);
    } else {
      this.camera = new THREE.PerspectiveCamera(38, width / height || 1, 0.1, 100);
      this.camera.position.set(1.45, 1.55, 2.35);
    }
    this.camera.lookAt(vec(RIG_FOCUS));

    if (orbit) {
      this.controls = new OrbitControls(this.camera, this.renderer.domElement);
      this.controls.enableDamping = true;
      this.controls.dampingFactor = 0.075;
      this.controls.enablePan = false;
      this.controls.minDistance = 1.1;
      this.controls.maxDistance = 4.5;
      this.controls.minPolarAngle = Math.PI * 0.12;
      this.controls.maxPolarAngle = Math.PI * 0.62;
      this.controls.target.copy(vec(RIG_FOCUS));
      this.controls.addEventListener('change', this.markDirty);
      this.controls.update();
    }

    this.addLights(style, ground);
    if (ground) this.addGround();
    this.addBones(style);

    this.selectionRing = this.buildSelectionRing();
    this.scene.add(this.selectionRing);

    for (const joint of joints) this.addJoint(joint, style);

    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(container);
    this.resize();

    const canvas = this.renderer.domElement;
    canvas.addEventListener('pointermove', this.handlePointerMove);
    canvas.addEventListener('pointerdown', this.handlePointerDown);
    canvas.addEventListener('pointerleave', this.handlePointerLeave);

    this.frame = requestAnimationFrame(this.tick);
  }

  // ---------------------------------------------------------------- build

  private addLights(style: SceneStyle, ground: boolean) {
    if (style === 'schematic') {
      this.scene.add(new THREE.AmbientLight(0xffffff, 1.4));
      return;
    }

    this.scene.add(new THREE.HemisphereLight(0x9fb6cc, 0x0c0e12, 0.75));

    const key = new THREE.DirectionalLight(COLOR.key, 2.1);
    key.position.set(2.4, 4.2, 3);
    key.castShadow = ground;
    key.shadow.mapSize.set(1024, 1024);
    key.shadow.camera.near = 0.5;
    key.shadow.camera.far = 12;
    key.shadow.camera.left = -1.6;
    key.shadow.camera.right = 1.6;
    key.shadow.camera.top = 2.4;
    key.shadow.camera.bottom = -0.2;
    key.shadow.normalBias = 0.02;
    this.scene.add(key);

    const rim = new THREE.DirectionalLight(COLOR.rim, 1.1);
    rim.position.set(-3, 2, -2.5);
    this.scene.add(rim);
  }

  private addGround() {
    const grid = new THREE.GridHelper(6, 24, COLOR.grid, COLOR.grid);
    const gridMaterial = grid.material as THREE.Material;
    gridMaterial.transparent = true;
    gridMaterial.opacity = 0.5;
    this.scene.add(grid);
    this.disposables.push(grid.geometry, gridMaterial);

    const shadowGeometry = new THREE.PlaneGeometry(6, 6);
    const shadowMaterial = new THREE.ShadowMaterial({ opacity: 0.45 });
    const shadowPlane = new THREE.Mesh(shadowGeometry, shadowMaterial);
    shadowPlane.rotation.x = -Math.PI / 2;
    shadowPlane.receiveShadow = true;
    this.scene.add(shadowPlane);
    this.disposables.push(shadowGeometry, shadowMaterial);
  }

  private addBones(style: SceneStyle) {
    const schematic = style === 'schematic';

    // One material per limb so limb focus can dim independently.
    const boneMaterials = new Map<Limb, THREE.Material>();
    const edgeMaterials = new Map<Limb, THREE.LineBasicMaterial>();
    const materialFor = (limb: Limb) => {
      const existing = boneMaterials.get(limb);
      if (existing) return existing;
      const created = schematic
        ? new THREE.MeshBasicMaterial({ color: COLOR.boneSchematic })
        : new THREE.MeshStandardMaterial({
            color: COLOR.bone,
            roughness: 0.62,
            metalness: 0.35,
          });
      boneMaterials.set(limb, created);
      this.disposables.push(created);
      return created;
    };
    const edgeMaterialFor = (limb: Limb) => {
      const existing = edgeMaterials.get(limb);
      if (existing) return existing;
      const created = new THREE.LineBasicMaterial({ color: COLOR.edge });
      edgeMaterials.set(limb, created);
      this.disposables.push(created);
      return created;
    };

    const up = new THREE.Vector3(0, 1, 0);

    for (const bone of BONES) {
      const material = materialFor(bone.limb);
      const from = vec(ANCHORS[bone.from]);
      const to = vec(ANCHORS[bone.to]);
      const direction = to.clone().sub(from);
      const distance = direction.length();
      const geometry = new THREE.CapsuleGeometry(
        bone.radius,
        Math.max(distance - bone.radius * 2, 0.001),
        4,
        schematic ? 8 : 16,
      );
      this.disposables.push(geometry);

      const mesh = new THREE.Mesh(geometry, material);
      mesh.position.copy(from).add(to).multiplyScalar(0.5);
      mesh.quaternion.setFromUnitVectors(up, direction.normalize());
      mesh.castShadow = !schematic;
      mesh.receiveShadow = !schematic;
      this.scene.add(mesh);
      this.boneMeshes.push({ mesh, limb: bone.limb });

      if (schematic) {
        const edges = new THREE.EdgesGeometry(geometry, 24);
        const outline = new THREE.LineSegments(edges, edgeMaterialFor(bone.limb));
        outline.position.copy(mesh.position);
        outline.quaternion.copy(mesh.quaternion);
        this.scene.add(outline);
        this.boneMeshes.push({ mesh: outline, limb: bone.limb });
        this.disposables.push(edges);
      }
    }
  }

  private buildSelectionRing(): THREE.Mesh {
    const geometry = new THREE.RingGeometry(0.055, 0.068, 48);
    const material = new THREE.MeshBasicMaterial({
      color: COLOR.accent,
      side: THREE.DoubleSide,
      transparent: true,
      opacity: 0.95,
    });
    this.disposables.push(geometry, material);

    const ring = new THREE.Mesh(geometry, material);
    ring.visible = false;
    ring.renderOrder = 2;
    return ring;
  }

  private addJoint(joint: ProtoJoint, style: SceneStyle) {
    const position = vec(jointPosition(joint));
    const baseColor = new THREE.Color(statusColor(joint));

    const geometry =
      style === 'schematic'
        ? new THREE.CircleGeometry(0.028, 24)
        : new THREE.SphereGeometry(0.028, 20, 20);
    const material =
      style === 'schematic'
        ? new THREE.MeshBasicMaterial({ color: baseColor })
        : new THREE.MeshStandardMaterial({
            color: baseColor,
            emissive: baseColor,
            emissiveIntensity: 0.35,
            roughness: 0.3,
            metalness: 0.1,
          });
    this.disposables.push(geometry, material);

    const marker = new THREE.Mesh(geometry, material);
    marker.position.copy(position);
    marker.renderOrder = 1;
    this.scene.add(marker);

    // Invisible, generous hit target — clicking a 28mm sphere is miserable.
    const hitGeometry = new THREE.SphereGeometry(0.075, 12, 12);
    const hitMaterial = new THREE.MeshBasicMaterial({ visible: false });
    this.disposables.push(hitGeometry, hitMaterial);
    const hitTarget = new THREE.Mesh(hitGeometry, hitMaterial);
    hitTarget.position.copy(position);
    hitTarget.userData.jointId = joint.id;
    this.scene.add(hitTarget);
    this.hitTargets.push(hitTarget);

    const label = document.createElement('div');
    label.textContent = joint.label;
    label.style.cssText = [
      'position:absolute',
      'transform:translate(0,-50%)',
      'white-space:nowrap',
      'font-family:var(--font-mono, monospace)',
      'font-size:10px',
      'letter-spacing:0.08em',
      'text-transform:uppercase',
      'padding:2px 6px',
      'border:1px solid var(--line-strong)',
      'background:color-mix(in oklab, var(--surface-0) 88%, transparent)',
      'color:var(--foreground)',
      'opacity:0',
      'transition:opacity 120ms linear',
    ].join(';');
    this.overlay.appendChild(label);

    this.visuals.set(joint.id, { joint, marker, hitTarget, label, baseColor, position });
  }

  // --------------------------------------------------------------- public

  setSelected(id: string | null) {
    if (this.selectedId === id) return;
    this.selectedId = id;
    this.applyJointState();
  }

  /** Dim every limb except this one; null restores full contrast. */
  setFocusLimb(limb: Limb | null) {
    if (this.focusLimb === limb) return;
    this.focusLimb = limb;
    this.applyJointState();
  }

  resize() {
    const width = this.container.clientWidth;
    const height = this.container.clientHeight;
    if (width === 0 || height === 0) return;

    if (this.camera instanceof THREE.PerspectiveCamera) {
      this.camera.aspect = width / height;
    } else {
      const frustumHeight = 2.1;
      const aspect = width / height;
      this.camera.left = (-frustumHeight * aspect) / 2;
      this.camera.right = (frustumHeight * aspect) / 2;
      this.camera.top = frustumHeight / 2;
      this.camera.bottom = -frustumHeight / 2;
    }
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(width, height, false);
    this.markDirty();
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;

    cancelAnimationFrame(this.frame);
    this.resizeObserver.disconnect();

    const canvas = this.renderer.domElement;
    canvas.removeEventListener('pointermove', this.handlePointerMove);
    canvas.removeEventListener('pointerdown', this.handlePointerDown);
    canvas.removeEventListener('pointerleave', this.handlePointerLeave);
    this.controls?.removeEventListener('change', this.markDirty);
    this.controls?.dispose();

    for (const disposable of this.disposables) disposable.dispose();
    this.renderer.dispose();

    canvas.remove();
    this.overlay.remove();
    document.body.style.cursor = '';
  }

  // -------------------------------------------------------------- internal

  private markDirty = () => {
    this.dirty = true;
  };

  private applyJointState() {
    for (const visual of this.visuals.values()) {
      const isSelected = visual.joint.id === this.selectedId;
      const isHovered = visual.joint.id === this.hoveredId;
      const dimmed =
        this.focusLimb !== null && visual.joint.limb !== this.focusLimb && !isSelected;

      const scale = isSelected ? 1.5 : isHovered ? 1.25 : 1;
      visual.marker.scale.setScalar(scale);

      const material = visual.marker.material as
        | THREE.MeshStandardMaterial
        | THREE.MeshBasicMaterial;
      material.color.copy(visual.baseColor);
      if (dimmed) material.color.multiplyScalar(0.4);
      if ('emissiveIntensity' in material) {
        material.emissiveIntensity = isSelected ? 0.9 : isHovered ? 0.6 : 0.35;
      }

      visual.label.style.opacity = isSelected || isHovered ? '1' : '0';
    }

    for (const { mesh, limb } of this.boneMeshes) {
      const dimmed = this.focusLimb !== null && limb !== this.focusLimb;
      const material = (mesh as THREE.Mesh).material as THREE.Material;
      material.transparent = true;
      material.opacity = dimmed ? 0.25 : 1;
    }

    const selected = this.selectedId ? this.visuals.get(this.selectedId) : undefined;
    this.selectionRing.visible = Boolean(selected);
    if (selected) this.selectionRing.position.copy(selected.position);

    this.markDirty();
  }

  private updatePointer(event: PointerEvent) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.pointer.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    this.pointer.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
  }

  private pick(): string | null {
    this.raycaster.setFromCamera(this.pointer, this.camera);
    const hits = this.raycaster.intersectObjects(this.hitTargets, false);
    return hits.length > 0 ? (hits[0].object.userData.jointId as string) : null;
  }

  private handlePointerMove = (event: PointerEvent) => {
    const now = performance.now();
    if (now - this.lastHoverTest < 40) return;
    this.lastHoverTest = now;

    this.updatePointer(event);
    const hovered = this.pick();
    if (hovered === this.hoveredId) return;

    this.hoveredId = hovered;
    this.renderer.domElement.style.cursor = hovered ? 'pointer' : 'default';
    this.onHover?.(hovered);
    this.applyJointState();
  };

  private handlePointerDown = (event: PointerEvent) => {
    this.updatePointer(event);
    this.onSelect(this.pick());
  };

  private handlePointerLeave = () => {
    if (this.hoveredId === null) return;
    this.hoveredId = null;
    this.renderer.domElement.style.cursor = 'default';
    this.onHover?.(null);
    this.applyJointState();
  };

  private syncLabels() {
    const width = this.container.clientWidth;
    const height = this.container.clientHeight;
    const projected = new THREE.Vector3();

    for (const visual of this.visuals.values()) {
      if (visual.label.style.opacity === '0') continue;
      projected.copy(visual.position).project(this.camera);
      const x = ((projected.x + 1) / 2) * width;
      const y = ((1 - projected.y) / 2) * height;
      visual.label.style.left = `${x + 14}px`;
      visual.label.style.top = `${y}px`;
    }
  }

  private tick = () => {
    this.frame = requestAnimationFrame(this.tick);
    if (this.controls?.enableDamping) this.controls.update();
    if (!this.dirty) return;
    this.dirty = false;

    this.selectionRing.quaternion.copy(this.camera.quaternion);
    this.renderer.render(this.scene, this.camera);
    this.syncLabels();
  };
}
