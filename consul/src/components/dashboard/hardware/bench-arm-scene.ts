/**
 * Bench arm schematic — vanilla Three.js joint picker (read-only visualization).
 * Patterns from prototype-hardware: dirty-flag render loop, raycast pick, dispose on teardown.
 */
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

import type { HardwareJointRow } from '@/components/dashboard/hardware/build-hardware-rows';

const COLOR = {
  void: 0x0a0a0a,
  grid: 0x22262c,
  accent: 0xffb000,
  ok: 0x2fd39b,
  neutral: 0x7c838d,
  arm: 0xa8b8c4,
} as const;

const MARKER_RADIUS = 0.06;

/** Approximate bench arm layout (+Y up, arm along +X). */
const BENCH_JOINT_OFFSETS: Record<string, THREE.Vector3> = {
  right_shoulder_roll: new THREE.Vector3(0, 0.9, 0),
  right_shoulder_pitch: new THREE.Vector3(0.15, 0.85, 0),
  right_upper_arm_yaw: new THREE.Vector3(0.3, 0.8, 0),
  right_elbow_pitch: new THREE.Vector3(0.45, 0.65, 0),
};

function defaultOffset(index: number): THREE.Vector3 {
  return new THREE.Vector3(0.1 * index, 0.9 - 0.05 * index, 0);
}

function statusColor(row: HardwareJointRow): number {
  if (row.warningCount > 0) {
    return COLOR.accent;
  }
  if (row.onCan) {
    return COLOR.ok;
  }
  return COLOR.neutral;
}

type JointVisual = {
  row: HardwareJointRow;
  marker: THREE.Mesh;
  baseColor: THREE.Color;
};

export type BenchArmSceneOptions = {
  container: HTMLElement;
  rows: HardwareJointRow[];
  onSelect: (joint: string | null) => void;
};

export class BenchArmScene {
  private readonly container: HTMLElement;
  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene = new THREE.Scene();
  private readonly camera: THREE.PerspectiveCamera;
  private readonly controls: OrbitControls;
  private readonly raycaster = new THREE.Raycaster();
  private readonly pointer = new THREE.Vector2();
  private readonly resizeObserver: ResizeObserver;

  private readonly visuals = new Map<string, JointVisual>();
  private readonly hitTargets: THREE.Mesh[] = [];
  private readonly disposables: { dispose: () => void }[] = [];

  private readonly onSelect: (joint: string | null) => void;
  private selectedJoint: string | null = null;
  private dirty = true;
  private disposed = false;

  constructor(options: BenchArmSceneOptions) {
    const { container, rows, onSelect } = options;
    this.container = container;
    this.onSelect = onSelect;

    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.setClearColor(COLOR.void, 1);
    container.appendChild(this.renderer.domElement);

    this.camera = new THREE.PerspectiveCamera(42, 1, 0.1, 50);
    this.camera.position.set(0.8, 1.0, 1.2);
    this.camera.lookAt(0.25, 0.75, 0);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.target.set(0.25, 0.75, 0);
    this.controls.enableDamping = true;
    this.controls.addEventListener('change', () => {
      this.dirty = true;
    });

    const ambient = new THREE.AmbientLight(0xffffff, 0.55);
    this.scene.add(ambient);
    const key = new THREE.DirectionalLight(0xffffff, 1.1);
    key.position.set(2, 3, 2);
    this.scene.add(key);

    const grid = new THREE.GridHelper(2, 20, COLOR.grid, COLOR.grid);
    grid.position.y = 0;
    this.scene.add(grid);

    const armGeom = new THREE.CylinderGeometry(0.04, 0.04, 0.5, 12);
    const armMat = new THREE.MeshStandardMaterial({ color: COLOR.arm, metalness: 0.3 });
    const arm = new THREE.Mesh(armGeom, armMat);
    arm.position.set(0.22, 0.78, 0);
    arm.rotation.z = Math.PI / 2;
    this.scene.add(arm);
    this.disposables.push(armGeom, armMat);

    const markerGeom = new THREE.SphereGeometry(MARKER_RADIUS, 16, 16);
    rows.forEach((row, index) => {
      const offset =
        BENCH_JOINT_OFFSETS[row.joint] ?? defaultOffset(index);
      const color = statusColor(row);
      const mat = new THREE.MeshStandardMaterial({
        color,
        emissive: new THREE.Color(color),
        emissiveIntensity: 0.35,
      });
      const marker = new THREE.Mesh(markerGeom, mat);
      marker.position.copy(offset);
      marker.userData.joint = row.joint;
      this.scene.add(marker);
      this.hitTargets.push(marker);
      this.visuals.set(row.joint, {
        row,
        marker,
        baseColor: new THREE.Color(color),
      });
      this.disposables.push(mat);
    });
    this.disposables.push(markerGeom);

    this.renderer.domElement.addEventListener('pointerdown', this.onPointer);
    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(container);
    this.resize();

    this.renderer.setAnimationLoop(() => {
      if (this.disposed) {
        return;
      }
      this.controls.update();
      if (this.dirty) {
        this.renderer.render(this.scene, this.camera);
        this.dirty = false;
      }
    });
  }

  private resize = () => {
    const { clientWidth, clientHeight } = this.container;
    if (clientWidth <= 0 || clientHeight <= 0) {
      return;
    }
    this.camera.aspect = clientWidth / clientHeight;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(clientWidth, clientHeight, false);
    this.dirty = true;
  };

  private onPointer = (event: PointerEvent) => {
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.pointer.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    this.pointer.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
    this.raycaster.setFromCamera(this.pointer, this.camera);
    const hits = this.raycaster.intersectObjects(this.hitTargets, false);
    const joint =
      hits.length > 0
        ? (hits[0].object.userData.joint as string | undefined)
        : undefined;
    this.onSelect(joint ?? null);
  };

  setSelected(joint: string | null) {
    this.selectedJoint = joint;
    for (const [id, visual] of this.visuals) {
      const mat = visual.marker.material as THREE.MeshStandardMaterial;
      if (id === joint) {
        mat.emissiveIntensity = 0.85;
        visual.marker.scale.setScalar(1.35);
      } else {
        mat.emissiveIntensity = 0.35;
        visual.marker.scale.setScalar(1);
      }
      mat.needsUpdate = true;
    }
    this.dirty = true;
  }

  dispose() {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.renderer.setAnimationLoop(null);
    this.resizeObserver.disconnect();
    this.renderer.domElement.removeEventListener('pointerdown', this.onPointer);
    this.controls.dispose();
    for (const item of this.disposables) {
      item.dispose();
    }
    this.renderer.dispose();
    if (this.renderer.domElement.parentElement === this.container) {
      this.container.removeChild(this.renderer.domElement);
    }
  }
}
