/**
 * PROTOTYPE — vanilla Three.js Bender-style joint picker.
 *
 * Throwaway homage for Wayfinder reaction, not production art.
 * Patterns from sickn33/antigravity-awesome-skills `threejs-skills`:
 * scene/camera/renderer, lit materials, OrbitControls, raycast picking,
 * soft shadows, ACES tone mapping, setAnimationLoop, dispose on teardown.
 */
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

import {
  ANCHORS,
  BENDER,
  MARKER_RADIUS,
  RIG_CENTER,
  RIG_HEIGHT,
  RIG_WIDTH,
  type Limb,
  type Vec3,
} from './humanoid-rig';
import { jointPosition, type ProtoJoint } from './mock-hardware';

const COLOR = {
  void: 0x0a0a0a,
  grid: 0x22262c,
  accent: 0xffb000,
  ok: 0x2fd39b,
  neutral: 0x7c838d,
  key: 0xffffff,
} as const;

export type SceneStyle = 'solid' | 'schematic';

export type HumanoidSceneOptions = {
  container: HTMLElement;
  joints: ProtoJoint[];
  style?: SceneStyle;
  orbit?: boolean;
  ground?: boolean;
  onSelect: (id: string | null) => void;
  onHover?: (id: string | null) => void;
};

type JointVisual = {
  joint: ProtoJoint;
  marker: THREE.Mesh;
  label: HTMLDivElement;
  baseColor: THREE.Color;
  position: THREE.Vector3;
};

type LimbPart = { mesh: THREE.Object3D; limb: Limb };

const vec = (v: Vec3) => new THREE.Vector3(v[0], v[1], v[2]);

function statusColor(joint: ProtoJoint): number {
  if (joint.completenessWarn) return COLOR.accent;
  if (joint.onCan) return COLOR.ok;
  return COLOR.neutral;
}

/** Corrugated dryer-hose limb (Bender's arms/legs). */
function corrugatedCylinder(
  radius: number,
  height: number,
  ridges = 12,
  radial = 18,
): THREE.CylinderGeometry {
  const geometry = new THREE.CylinderGeometry(radius, radius, height, radial, ridges * 2);
  const position = geometry.attributes.position;
  for (let i = 0; i < position.count; i += 1) {
    const x = position.getX(i);
    const y = position.getY(i);
    const z = position.getZ(i);
    const angle = Math.atan2(z, x);
    const t = (y + height / 2) / height;
    const wave = 1 + 0.32 * Math.sin(t * ridges * Math.PI * 2);
    position.setXYZ(i, Math.cos(angle) * radius * wave, y, Math.sin(angle) * radius * wave);
  }
  position.needsUpdate = true;
  geometry.computeVertexNormals();
  return geometry;
}

export class HumanoidScene {
  private readonly container: HTMLElement;
  private readonly overlay: HTMLDivElement;
  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene = new THREE.Scene();
  private readonly camera: THREE.PerspectiveCamera | THREE.OrthographicCamera;
  private readonly robot = new THREE.Group();
  private readonly controls: OrbitControls | null = null;
  private readonly raycaster = new THREE.Raycaster();
  private readonly pointer = new THREE.Vector2();
  private readonly resizeObserver: ResizeObserver;

  private readonly visuals = new Map<string, JointVisual>();
  private readonly hitTargets: THREE.Mesh[] = [];
  private readonly limbParts: LimbPart[] = [];
  private readonly selectionRing: THREE.Mesh;
  private readonly disposables: { dispose: () => void }[] = [];
  private readonly materials = new Map<string, THREE.Material>();

  private readonly onSelect: (id: string | null) => void;
  private readonly onHover: ((id: string | null) => void) | undefined;
  private readonly style: SceneStyle;

  private selectedId: string | null = null;
  private hoveredId: string | null = null;
  private focusLimb: Limb | null = null;
  private lastHoverTest = 0;
  private dirty = true;
  private disposed = false;
  private userFramed = false;

  constructor(options: HumanoidSceneOptions) {
    const { container, joints, style = 'solid', orbit = true, ground = true } = options;

    this.container = container;
    this.onSelect = options.onSelect;
    this.onHover = options.onHover;
    this.style = style;

    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
      powerPreference: 'high-performance',
    });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.25;
    this.renderer.shadowMap.enabled = ground && style === 'solid';
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.renderer.setClearColor(COLOR.void, 1);
    this.renderer.domElement.style.display = 'block';
    this.renderer.domElement.style.width = '100%';
    this.renderer.domElement.style.height = '100%';
    this.renderer.domElement.style.touchAction = 'none';
    container.appendChild(this.renderer.domElement);

    this.overlay = document.createElement('div');
    this.overlay.style.cssText =
      'position:absolute;inset:0;pointer-events:none;overflow:hidden;';
    container.appendChild(this.overlay);

    if (orbit) {
      this.camera = new THREE.PerspectiveCamera(32, 1, 0.05, 100);
      this.camera.position.set(1.5, RIG_CENTER[1] + 0.2, 2.7);
    } else {
      this.camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.01, 100);
      this.camera.position.set(0, RIG_CENTER[1], 6);
    }
    this.camera.lookAt(vec(RIG_CENTER));

    if (orbit) {
      this.controls = new OrbitControls(this.camera, this.renderer.domElement);
      this.controls.enableDamping = true;
      this.controls.dampingFactor = 0.08;
      this.controls.enablePan = false;
      this.controls.minDistance = 1.5;
      this.controls.maxDistance = 5.5;
      this.controls.minPolarAngle = Math.PI * 0.18;
      this.controls.maxPolarAngle = Math.PI * 0.58;
      this.controls.target.copy(vec(RIG_CENTER));
      this.controls.addEventListener('change', this.markDirty);
      this.controls.addEventListener('start', this.markUserFramed);
      this.controls.update();
    }

    this.scene.add(this.robot);
    this.addLights(ground);
    if (ground) this.addGround();
    this.buildBender();

    this.selectionRing = this.buildSelectionRing();
    this.scene.add(this.selectionRing);

    for (const joint of joints) this.addJointMarker(joint);

    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(container);
    this.resize();

    const canvas = this.renderer.domElement;
    canvas.addEventListener('pointermove', this.handlePointerMove);
    canvas.addEventListener('pointerdown', this.handlePointerDown);
    canvas.addEventListener('pointerleave', this.handlePointerLeave);

    this.renderer.setAnimationLoop(this.tick);
  }

  // ---------------------------------------------------------------- mats

  private mat(
    key: string,
    opts: ConstructorParameters<typeof THREE.MeshStandardMaterial>[0] & {
      basic?: boolean;
    },
  ): THREE.Material {
    const existing = this.materials.get(key);
    if (existing) return existing;

    const material =
      this.style === 'schematic' || opts.basic
        ? new THREE.MeshBasicMaterial({
            color: opts.color,
          })
        : new THREE.MeshStandardMaterial({
            color: opts.color,
            roughness: opts.roughness ?? 0.45,
            metalness: opts.metalness ?? 0.65,
            ...(opts.emissive !== undefined ? { emissive: opts.emissive } : {}),
            ...(opts.emissiveIntensity !== undefined
              ? { emissiveIntensity: opts.emissiveIntensity }
              : {}),
          });
    this.materials.set(key, material);
    this.disposables.push(material);
    return material;
  }

  private addPart(
    geometry: THREE.BufferGeometry,
    material: THREE.Material,
    position: THREE.Vector3,
    limb: Limb,
    rotation?: THREE.Euler,
  ): THREE.Mesh {
    this.disposables.push(geometry);
    // Clone so limb focus can dim opacity without affecting shared mats.
    const owned = material.clone();
    this.disposables.push(owned);
    const mesh = new THREE.Mesh(geometry, owned);
    mesh.position.copy(position);
    if (rotation) mesh.rotation.copy(rotation);
    mesh.castShadow = this.style === 'solid';
    mesh.receiveShadow = this.style === 'solid';
    this.robot.add(mesh);
    this.limbParts.push({ mesh, limb });

    if (this.style === 'schematic') {
      const edges = new THREE.EdgesGeometry(geometry, 20);
      const lineMat = new THREE.LineBasicMaterial({ color: BENDER.cream });
      const outline = new THREE.LineSegments(edges, lineMat);
      outline.position.copy(position);
      if (rotation) outline.rotation.copy(rotation);
      this.robot.add(outline);
      this.limbParts.push({ mesh: outline, limb });
      this.disposables.push(edges, lineMat);
    }

    return mesh;
  }

  // ---------------------------------------------------------------- build

  private addLights(ground: boolean) {
    if (this.style === 'schematic') {
      this.scene.add(new THREE.AmbientLight(0xffffff, 1.4));
      return;
    }

    this.scene.add(new THREE.AmbientLight(0xffffff, 0.45));
    this.scene.add(new THREE.HemisphereLight(0xd0d8e0, 0x101418, 0.55));

    const key = new THREE.DirectionalLight(COLOR.key, 1.6);
    key.position.set(2.4, 4.2, 3);
    key.castShadow = ground;
    key.shadow.mapSize.set(1024, 1024);
    key.shadow.camera.near = 0.5;
    key.shadow.camera.far = 14;
    key.shadow.camera.left = -2;
    key.shadow.camera.right = 2;
    key.shadow.camera.top = 2.6;
    key.shadow.camera.bottom = -0.2;
    key.shadow.normalBias = 0.02;
    this.scene.add(key);

    const fill = new THREE.DirectionalLight(0xffffff, 0.45);
    fill.position.set(-2, 2, 2);
    this.scene.add(fill);
  }

  private addGround() {
    const grid = new THREE.GridHelper(5, 20, COLOR.grid, COLOR.grid);
    const gridMaterial = grid.material as THREE.Material;
    gridMaterial.transparent = true;
    gridMaterial.opacity = 0.35;
    this.scene.add(grid);
    this.disposables.push(grid.geometry, gridMaterial);

    if (this.style === 'solid') {
      const shadowGeometry = new THREE.CircleGeometry(1.2, 48);
      const shadowMaterial = new THREE.ShadowMaterial({ opacity: 0.55 });
      const shadowPlane = new THREE.Mesh(shadowGeometry, shadowMaterial);
      shadowPlane.rotation.x = -Math.PI / 2;
      shadowPlane.position.y = 0.001;
      shadowPlane.receiveShadow = true;
      this.scene.add(shadowPlane);
      this.disposables.push(shadowGeometry, shadowMaterial);
    }
  }

  private buildBender() {
    const metal = this.mat('metal', {
      color: BENDER.metal,
      roughness: 0.4,
      metalness: 0.7,
    });
    const metalDark = this.mat('metal-dark', {
      color: BENDER.metalDark,
      roughness: 0.45,
      metalness: 0.75,
    });
    const cream = this.mat('cream', {
      color: BENDER.cream,
      roughness: 0.55,
      metalness: 0.1,
      emissive: this.style === 'solid' ? BENDER.cream : undefined,
      emissiveIntensity: this.style === 'solid' ? 0.15 : undefined,
    });
    const pupil = this.mat('pupil', {
      color: BENDER.pupil,
      roughness: 0.8,
      metalness: 0.1,
    });

    // --- Head (cylinder + dome + antenna) ---
    const headY = 1.48;
    this.addPart(
      new THREE.CylinderGeometry(0.16, 0.16, 0.3, 32),
      metal,
      new THREE.Vector3(0, headY, 0),
      'torso',
    );
    this.addPart(
      new THREE.SphereGeometry(0.16, 32, 20, 0, Math.PI * 2, 0, Math.PI / 2),
      metal,
      new THREE.Vector3(0, headY + 0.15, 0),
      'torso',
    );
    // Antenna
    this.addPart(
      new THREE.CylinderGeometry(0.014, 0.02, 0.11, 12),
      metalDark,
      new THREE.Vector3(0, headY + 0.32, 0),
      'torso',
    );
    this.addPart(
      new THREE.SphereGeometry(0.03, 14, 12),
      metal,
      new THREE.Vector3(0, headY + 0.39, 0),
      'torso',
    );

    // Visor housing
    this.addPart(
      new THREE.BoxGeometry(0.24, 0.11, 0.09),
      metalDark,
      new THREE.Vector3(0, headY + 0.02, 0.125),
      'torso',
    );
    // Eyes — cream discs + square pupils
    for (const x of [-0.055, 0.055]) {
      this.addPart(
        new THREE.CircleGeometry(0.042, 24),
        cream,
        new THREE.Vector3(x, headY + 0.02, 0.172),
        'torso',
      );
      this.addPart(
        new THREE.PlaneGeometry(0.018, 0.018),
        pupil,
        new THREE.Vector3(x, headY + 0.02, 0.174),
        'torso',
      );
    }

    // Mouth grill
    this.addPart(
      new THREE.BoxGeometry(0.15, 0.075, 0.045),
      metalDark,
      new THREE.Vector3(0, headY - 0.1, 0.135),
      'torso',
    );
    for (let row = 0; row < 2; row += 1) {
      for (let col = 0; col < 4; col += 1) {
        this.addPart(
          new THREE.PlaneGeometry(0.026, 0.022),
          cream,
          new THREE.Vector3(-0.048 + col * 0.032, headY - 0.085 - row * 0.03, 0.16),
          'torso',
        );
      }
    }

    // Neck stub so the head isn't floating
    this.addPart(
      new THREE.CylinderGeometry(0.07, 0.09, 0.08, 20),
      metalDark,
      new THREE.Vector3(0, 1.28, 0),
      'torso',
    );

    // --- Torso (slightly tapered cylinder + chest door) ---
    this.addPart(
      new THREE.CylinderGeometry(0.22, 0.26, 0.55, 32),
      metal,
      new THREE.Vector3(0, 0.98, 0),
      'torso',
    );
    // Chest door
    this.addPart(
      new THREE.BoxGeometry(0.18, 0.22, 0.04),
      metalDark,
      new THREE.Vector3(0, 1.05, 0.2),
      'torso',
    );
    this.addPart(
      new THREE.SphereGeometry(0.018, 12, 10),
      this.mat('knob', { color: BENDER.doorKnob, roughness: 0.5, metalness: 0.8 }),
      new THREE.Vector3(0.07, 1.05, 0.225),
      'torso',
    );

    // Shoulder balls
    for (const side of [-1, 1] as const) {
      const limb: Limb = side > 0 ? 'right_arm' : 'left_arm';
      this.addPart(
        new THREE.SphereGeometry(0.08, 20, 16),
        metal,
        new THREE.Vector3(side * 0.26, 1.22, 0),
        limb,
      );
    }

    // Arms — corrugated tubes
    this.buildLimb('right_arm', [
      [ANCHORS.shoulder_r, ANCHORS.elbow_r, 0.055],
      [ANCHORS.elbow_r, ANCHORS.wrist_r, 0.05],
    ]);
    this.buildLimb('left_arm', [
      [ANCHORS.shoulder_l, ANCHORS.elbow_l, 0.055],
      [ANCHORS.elbow_l, ANCHORS.wrist_l, 0.05],
    ]);

    // Hands — paddle + fingers
    for (const side of [-1, 1] as const) {
      const limb: Limb = side > 0 ? 'right_arm' : 'left_arm';
      const wrist = vec(side > 0 ? ANCHORS.wrist_r : ANCHORS.wrist_l);
      this.addPart(
        new THREE.BoxGeometry(0.1, 0.04, 0.08),
        metal,
        wrist.clone().add(new THREE.Vector3(0, -0.04, 0.01)),
        limb,
      );
      for (let f = 0; f < 3; f += 1) {
        this.addPart(
          new THREE.BoxGeometry(0.025, 0.07, 0.025),
          metal,
          wrist.clone().add(new THREE.Vector3(side * (-0.03 + f * 0.03), -0.1, 0.02)),
          limb,
        );
      }
    }

    // Hip balls + legs
    for (const side of [-1, 1] as const) {
      const limb: Limb = side > 0 ? 'right_leg' : 'left_leg';
      this.addPart(
        new THREE.SphereGeometry(0.07, 18, 14),
        metal,
        new THREE.Vector3(side * 0.1, 0.72, 0),
        limb,
      );
    }
    this.buildLimb('right_leg', [
      [ANCHORS.hip_r, ANCHORS.knee_r, 0.06],
      [ANCHORS.knee_r, ANCHORS.ankle_r, 0.055],
    ]);
    this.buildLimb('left_leg', [
      [ANCHORS.hip_l, ANCHORS.knee_l, 0.06],
      [ANCHORS.knee_l, ANCHORS.ankle_l, 0.055],
    ]);

    // Dome feet — hemisphere with flat on the floor, dome up
    for (const side of [-1, 1] as const) {
      const limb: Limb = side > 0 ? 'right_leg' : 'left_leg';
      this.addPart(
        new THREE.SphereGeometry(0.13, 24, 16, 0, Math.PI * 2, 0, Math.PI / 2),
        metal,
        new THREE.Vector3(side * 0.12, 0.01, 0.03),
        limb,
      );
    }
  }

  private buildLimb(
    limb: Limb,
    segments: [Vec3, Vec3, number][],
  ) {
    const metal = this.mat('metal', {
      color: BENDER.metal,
      roughness: 0.4,
      metalness: 0.7,
    });
    const up = new THREE.Vector3(0, 1, 0);

    for (const [fromV, toV, radius] of segments) {
      const from = vec(fromV);
      const to = vec(toV);
      const direction = to.clone().sub(from);
      const length = direction.length();
      const geometry = corrugatedCylinder(radius, length, 10, 16);
      const mesh = this.addPart(
        geometry,
        metal,
        from.clone().add(to).multiplyScalar(0.5),
        limb,
      );
      mesh.quaternion.setFromUnitVectors(up, direction.normalize());
    }
  }

  private buildSelectionRing(): THREE.Mesh {
    const geometry = new THREE.TorusGeometry(MARKER_RADIUS + 0.02, 0.007, 10, 40);
    const material = new THREE.MeshBasicMaterial({
      color: COLOR.accent,
      transparent: true,
      opacity: 0.95,
      depthTest: false,
    });
    this.disposables.push(geometry, material);
    const ring = new THREE.Mesh(geometry, material);
    ring.visible = false;
    ring.renderOrder = 3;
    return ring;
  }

  private addJointMarker(joint: ProtoJoint) {
    const position = vec(jointPosition(joint));
    position.z += 0.05;
    const baseColor = new THREE.Color(statusColor(joint));

    const geometry =
      this.style === 'schematic'
        ? new THREE.CircleGeometry(MARKER_RADIUS * 0.9, 24)
        : new THREE.SphereGeometry(MARKER_RADIUS, 18, 14);
    const material =
      this.style === 'schematic'
        ? new THREE.MeshBasicMaterial({ color: baseColor })
        : new THREE.MeshStandardMaterial({
            color: baseColor,
            emissive: baseColor,
            emissiveIntensity: 0.65,
            roughness: 0.3,
            metalness: 0.15,
          });
    this.disposables.push(geometry, material);

    const marker = new THREE.Mesh(geometry, material);
    marker.position.copy(position);
    marker.renderOrder = 2;
    this.robot.add(marker);

    const hitGeometry = new THREE.SphereGeometry(MARKER_RADIUS * 2.8, 12, 10);
    const hitMaterial = new THREE.MeshBasicMaterial({ visible: false });
    this.disposables.push(hitGeometry, hitMaterial);
    const hitTarget = new THREE.Mesh(hitGeometry, hitMaterial);
    hitTarget.position.copy(position);
    hitTarget.userData.jointId = joint.id;
    this.robot.add(hitTarget);
    this.hitTargets.push(hitTarget);

    const label = document.createElement('div');
    label.textContent = joint.label;
    label.style.cssText = [
      'position:absolute',
      'transform:translate(0,-50%)',
      'white-space:nowrap',
      'font-family:var(--font-mono, monospace)',
      'font-size:10px',
      'letter-spacing:0.12em',
      'text-transform:uppercase',
      'padding:3px 7px',
      'border:1px solid var(--line-strong)',
      'background:var(--surface-0)',
      'color:var(--foreground)',
      'opacity:0',
      'transition:opacity 120ms linear',
    ].join(';');
    this.overlay.appendChild(label);

    this.visuals.set(joint.id, { joint, marker, label, baseColor, position });
  }

  // --------------------------------------------------------------- public

  setSelected(id: string | null) {
    if (this.selectedId === id) return;
    this.selectedId = id;
    this.applyJointState();
  }

  setFocusLimb(limb: Limb | null) {
    if (this.focusLimb === limb) return;
    this.focusLimb = limb;
    this.applyJointState();
  }

  resize() {
    const width = this.container.clientWidth;
    const height = this.container.clientHeight;
    if (width === 0 || height === 0) return;

    const aspect = width / height;
    const margin = 1.15;
    const fitHeight = RIG_HEIGHT * margin;
    const fitWidth = RIG_WIDTH * margin;

    if (this.camera instanceof THREE.PerspectiveCamera) {
      this.camera.aspect = aspect;
      const vFov = THREE.MathUtils.degToRad(this.camera.fov);
      const distanceForHeight = fitHeight / 2 / Math.tan(vFov / 2);
      const distanceForWidth = fitWidth / 2 / Math.tan(vFov / 2) / aspect;
      const distance = Math.max(distanceForHeight, distanceForWidth);

      if (this.controls) {
        this.controls.minDistance = distance * 0.6;
        this.controls.maxDistance = distance * 2.1;
        if (!this.userFramed) {
          const target = vec(RIG_CENTER);
          const offset = this.camera.position.clone().sub(this.controls.target).normalize();
          this.camera.position.copy(target).add(offset.multiplyScalar(distance));
          this.controls.update();
        }
      }
    } else {
      const frustumHeight = Math.max(fitHeight, fitWidth / aspect);
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

    this.renderer.setAnimationLoop(null);
    this.resizeObserver.disconnect();

    const canvas = this.renderer.domElement;
    canvas.removeEventListener('pointermove', this.handlePointerMove);
    canvas.removeEventListener('pointerdown', this.handlePointerDown);
    canvas.removeEventListener('pointerleave', this.handlePointerLeave);
    this.controls?.removeEventListener('change', this.markDirty);
    this.controls?.removeEventListener('start', this.markUserFramed);
    this.controls?.dispose();

    for (const disposable of this.disposables) disposable.dispose();
    this.renderer.dispose();

    canvas.remove();
    this.overlay.remove();
  }

  // -------------------------------------------------------------- internal

  private markUserFramed = () => {
    this.userFramed = true;
  };

  private markDirty = () => {
    this.dirty = true;
  };

  private applyJointState() {
    for (const visual of this.visuals.values()) {
      const isSelected = visual.joint.id === this.selectedId;
      const isHovered = visual.joint.id === this.hoveredId;
      const dimmed =
        this.focusLimb !== null && visual.joint.limb !== this.focusLimb && !isSelected;

      visual.marker.scale.setScalar(isSelected ? 1.25 : isHovered ? 1.12 : 1);

      const material = visual.marker.material as
        | THREE.MeshStandardMaterial
        | THREE.MeshBasicMaterial;
      material.color.copy(visual.baseColor);
      if (dimmed) material.color.multiplyScalar(0.35);
      if ('emissiveIntensity' in material) {
        material.emissiveIntensity = isSelected ? 1.1 : isHovered ? 0.85 : 0.65;
      }

      visual.label.style.opacity = isSelected || isHovered ? '1' : '0';
    }

    for (const { mesh, limb } of this.limbParts) {
      const dimmed = this.focusLimb !== null && limb !== this.focusLimb;
      const material = (mesh as THREE.Mesh).material as THREE.Material;
      if (material.transparent !== dimmed) {
        material.transparent = dimmed;
        material.needsUpdate = true;
      }
      material.opacity = dimmed ? 0.2 : 1;
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
    const picked = this.pick();
    if (picked !== null) this.onSelect(picked);
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
      const flip = x > width - 180;
      visual.label.style.left = flip ? 'auto' : `${x + 16}px`;
      visual.label.style.right = flip ? `${width - x + 16}px` : 'auto';
      visual.label.style.top = `${y}px`;
    }
  }

  private tick = () => {
    if (this.controls?.enableDamping) this.controls.update();
    if (!this.dirty) return;
    this.dirty = false;

    this.selectionRing.quaternion.copy(this.camera.quaternion);
    this.renderer.render(this.scene, this.camera);
    this.syncLabels();
  };
}
