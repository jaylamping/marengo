/**
 * PROTOTYPE — vanilla Three.js humanoid robot joint picker.
 *
 * Built against sickn33/antigravity-awesome-skills `threejs-skills`:
 * scene/camera/renderer, MeshStandardMaterial + lights, OrbitControls,
 * raycast picking, shadow maps, ACES tone mapping, dispose on teardown.
 *
 * No react-three-fiber. React mounts the viewport; this class owns WebGL.
 */
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

import {
  ANCHORS,
  ARMOR,
  JOINT_BALLS,
  MARKER_RADIUS,
  RIG_CENTER,
  RIG_HEIGHT,
  RIG_WIDTH,
  type Limb,
  type Vec3,
} from './humanoid-rig';
import { jointPosition, type ProtoJoint } from './mock-hardware';

const COLOR = {
  void: 0x121418,
  shell: 0xc8ccd2,
  metal: 0x2a2f36,
  bead: 0x1a1e24,
  edge: 0x6a7280,
  grid: 0x2a3038,
  accent: 0xffb000,
  ok: 0x2fd39b,
  neutral: 0x7c838d,
  visor: 0x1a3040,
  glow: 0x5ec8ff,
  key: 0xffffff,
  rim: 0x8fb4d6,
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

  private readonly onSelect: (id: string | null) => void;
  private readonly onHover: ((id: string | null) => void) | undefined;

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

    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
      powerPreference: 'high-performance',
    });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.1;
    this.renderer.shadowMap.enabled = ground && style === 'solid';
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.renderer.setClearColor(COLOR.void, style === 'schematic' ? 1 : 0.92);
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
      this.camera.position.set(1.35, RIG_CENTER[1] + 0.35, 2.5);
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
      this.controls.minDistance = 1.4;
      this.controls.maxDistance = 5.5;
      this.controls.minPolarAngle = Math.PI * 0.18;
      this.controls.maxPolarAngle = Math.PI * 0.58;
      this.controls.target.copy(vec(RIG_CENTER));
      this.controls.addEventListener('change', this.markDirty);
      this.controls.addEventListener('start', this.markUserFramed);
      this.controls.update();
    }

    this.scene.add(this.robot);
    this.addLights(style, ground);
    if (ground) this.addGround();
    this.addArmor(style);
    this.addJointBalls(style);

    this.selectionRing = this.buildSelectionRing();
    this.scene.add(this.selectionRing);

    for (const joint of joints) this.addJointMarker(joint, style);

    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(container);
    this.resize();

    const canvas = this.renderer.domElement;
    canvas.addEventListener('pointermove', this.handlePointerMove);
    canvas.addEventListener('pointerdown', this.handlePointerDown);
    canvas.addEventListener('pointerleave', this.handlePointerLeave);

    this.renderer.setAnimationLoop(this.tick);
  }

  // ---------------------------------------------------------------- build

  private addLights(style: SceneStyle, ground: boolean) {
    if (style === 'schematic') {
      this.scene.add(new THREE.AmbientLight(0xffffff, 1.6));
      return;
    }

    this.scene.add(new THREE.AmbientLight(0xffffff, 0.35));
    this.scene.add(new THREE.HemisphereLight(0xb8c8d8, 0x101218, 0.55));

    const key = new THREE.DirectionalLight(COLOR.key, 1.8);
    key.position.set(2.2, 4.5, 2.8);
    key.castShadow = ground;
    key.shadow.mapSize.set(1024, 1024);
    key.shadow.camera.near = 0.5;
    key.shadow.camera.far = 14;
    key.shadow.camera.left = -1.8;
    key.shadow.camera.right = 1.8;
    key.shadow.camera.top = 2.6;
    key.shadow.camera.bottom = -0.2;
    key.shadow.normalBias = 0.02;
    this.scene.add(key);

    const rim = new THREE.DirectionalLight(COLOR.rim, 1.0);
    rim.position.set(-2.8, 2.2, -2.2);
    this.scene.add(rim);

    const fill = new THREE.DirectionalLight(0xffffff, 0.35);
    fill.position.set(0, 1.5, 3);
    this.scene.add(fill);
  }

  private addGround() {
    const grid = new THREE.GridHelper(5, 20, COLOR.grid, COLOR.grid);
    const gridMaterial = grid.material as THREE.Material;
    gridMaterial.transparent = true;
    gridMaterial.opacity = 0.4;
    this.scene.add(grid);
    this.disposables.push(grid.geometry, gridMaterial);

    const shadowGeometry = new THREE.CircleGeometry(1.4, 48);
    const shadowMaterial = new THREE.ShadowMaterial({ opacity: 0.5 });
    const shadowPlane = new THREE.Mesh(shadowGeometry, shadowMaterial);
    shadowPlane.rotation.x = -Math.PI / 2;
    shadowPlane.position.y = 0.001;
    shadowPlane.receiveShadow = true;
    this.scene.add(shadowPlane);
    this.disposables.push(shadowGeometry, shadowMaterial);
  }

  private materialFor(
    kind: 'shell' | 'metal' | 'visor' | 'glow' | 'bead',
    style: SceneStyle,
  ): THREE.Material {
    if (style === 'schematic') {
      const material = new THREE.MeshBasicMaterial({
        color:
          kind === 'visor' || kind === 'glow'
            ? COLOR.edge
            : kind === 'metal' || kind === 'bead'
              ? 0x14181e
              : 0x1c222b,
      });
      this.disposables.push(material);
      return material;
    }

    const specs: Record<typeof kind, ConstructorParameters<typeof THREE.MeshStandardMaterial>[0]> =
      {
        shell: {
          color: COLOR.shell,
          roughness: 0.35,
          metalness: 0.55,
        },
        metal: {
          color: COLOR.metal,
          roughness: 0.4,
          metalness: 0.85,
        },
        bead: {
          color: COLOR.bead,
          roughness: 0.35,
          metalness: 0.9,
        },
        visor: {
          color: COLOR.visor,
          roughness: 0.15,
          metalness: 0.7,
          emissive: COLOR.glow,
          emissiveIntensity: 0.35,
        },
        glow: {
          color: COLOR.glow,
          roughness: 0.4,
          metalness: 0.2,
          emissive: COLOR.glow,
          emissiveIntensity: 0.7,
        },
      };

    const material = new THREE.MeshStandardMaterial(specs[kind]);
    this.disposables.push(material);
    return material;
  }

  private addArmor(style: SceneStyle) {
    const schematic = style === 'schematic';
    const mats = {
      shell: this.materialFor('shell', style),
      metal: this.materialFor('metal', style),
      visor: this.materialFor('visor', style),
      glow: this.materialFor('glow', style),
    };

    for (const plate of ARMOR) {
      const [sx, sy, sz] = plate.size;
      const shape = plate.shape ?? 'box';
      const geometry =
        shape === 'sphere'
          ? new THREE.SphereGeometry(Math.max(sx, sy, sz) / 2, schematic ? 14 : 24, 16)
          : shape === 'cylinder'
            ? new THREE.CylinderGeometry(sx / 2, sx / 2, sy, schematic ? 12 : 20)
            : new THREE.BoxGeometry(sx, sy, sz);
      this.disposables.push(geometry);

      const mesh = new THREE.Mesh(geometry, mats[plate.kind]);
      const base = vec(ANCHORS[plate.anchor]);
      if (plate.offset) {
        base.add(vec(plate.offset));
      }
      mesh.position.copy(base);
      if (plate.rotation) {
        mesh.rotation.set(plate.rotation[0], plate.rotation[1], plate.rotation[2]);
      }
      mesh.castShadow = !schematic;
      mesh.receiveShadow = !schematic;
      this.robot.add(mesh);
      this.limbParts.push({ mesh, limb: plate.limb });

      if (schematic) {
        const edges = new THREE.EdgesGeometry(geometry, 18);
        const edgeMat = new THREE.LineBasicMaterial({ color: COLOR.edge });
        const outline = new THREE.LineSegments(edges, edgeMat);
        outline.position.copy(mesh.position);
        outline.rotation.copy(mesh.rotation);
        this.robot.add(outline);
        this.limbParts.push({ mesh: outline, limb: plate.limb });
        this.disposables.push(edges, edgeMat);
      }
    }
  }

  private addJointBalls(style: SceneStyle) {
    const material = this.materialFor('bead', style);
    const schematic = style === 'schematic';

    for (const ball of JOINT_BALLS) {
      const geometry = new THREE.SphereGeometry(ball.radius, schematic ? 12 : 20, 14);
      this.disposables.push(geometry);
      const mesh = new THREE.Mesh(geometry, material);
      mesh.position.copy(vec(ANCHORS[ball.anchor]));
      mesh.castShadow = !schematic;
      this.robot.add(mesh);
      this.limbParts.push({ mesh, limb: ball.limb });
    }
  }

  private buildSelectionRing(): THREE.Mesh {
    const geometry = new THREE.TorusGeometry(MARKER_RADIUS + 0.018, 0.006, 10, 40);
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

  private addJointMarker(joint: ProtoJoint, style: SceneStyle) {
    const position = vec(jointPosition(joint));
    // Sit the marker slightly proud of the joint so it reads as a status LED.
    position.z += 0.04;
    const baseColor = new THREE.Color(statusColor(joint));

    const geometry =
      style === 'schematic'
        ? new THREE.CircleGeometry(MARKER_RADIUS * 0.85, 28)
        : new THREE.SphereGeometry(MARKER_RADIUS, 20, 16);
    const material =
      style === 'schematic'
        ? new THREE.MeshBasicMaterial({ color: baseColor })
        : new THREE.MeshStandardMaterial({
            color: baseColor,
            emissive: baseColor,
            emissiveIntensity: 0.55,
            roughness: 0.25,
            metalness: 0.2,
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
    const margin = 1.18;
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
        material.emissiveIntensity = isSelected ? 1.1 : isHovered ? 0.8 : 0.55;
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
      material.opacity = dimmed ? 0.22 : 1;
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
