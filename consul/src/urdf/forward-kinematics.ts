import * as THREE from 'three';
import type { RobotModel } from '@/urdf/parse-urdf';

/**
 * Compute forward kinematics for all links given joint positions.
 * Joint positions should be a map from jointName to angle (in radians).
 * Returns a map from linkName to the world-space THREE.Matrix4 for that link.
 */
export function computeForwardKinematics(
  model: RobotModel,
  jointPositions: Record<string, number>,
): Map<string, THREE.Matrix4> {
  const worldMatrices = new Map<string, THREE.Matrix4>();

  // Initialize root links with identity matrix
  for (const rootLink of model.rootLinks) {
    worldMatrices.set(rootLink, new THREE.Matrix4().identity());
  }

  // We need to traverse joints in a topological order (parents before children)
  // For a tree structure, we can just do a BFS starting from root links
  const queue: string[] = [...model.rootLinks];
  const visitedLinks = new Set<string>(model.rootLinks);

  while (queue.length > 0) {
    const currentLinkName = queue.shift()!;
    const currentMatrix = worldMatrices.get(currentLinkName)!;

    // Find all joints where currentLinkName is the parent
    for (const joint of model.joints.values()) {
      if (joint.parent === currentLinkName && !visitedLinks.has(joint.child)) {
        visitedLinks.add(joint.child);
        queue.push(joint.child);

        // Compute this joint's transformation
        const jointMatrix = computeJointTransform(joint, jointPositions[joint.name] || 0);

        // World transform of child = parentWorld * jointOrigin * jointRotation
        const childMatrix = new THREE.Matrix4().copy(currentMatrix).multiply(jointMatrix);
        worldMatrices.set(joint.child, childMatrix);
      }
    }
  }

  return worldMatrices;
}

function computeJointTransform(
  joint: {
    origin: { xyz: readonly [number, number, number]; rpy: readonly [number, number, number] };
    axis: readonly [number, number, number];
  },
  jointValue: number,
): THREE.Matrix4 {
  const matrix = new THREE.Matrix4();

  // 1. Apply origin translation
  const [ox, oy, oz] = joint.origin.xyz;
  matrix.makeTranslation(ox, oy, oz);

  // 2. Apply origin rotation (rpy = roll, pitch, yaw)
  // URDF uses intrinsic rotations in roll (X) -> pitch (Y) -> yaw (Z) order.
  // THREE.Euler default order 'XYZ' exactly matches this intrinsic sequence.
  const [rx, ry, rz] = joint.origin.rpy;
  const originEuler = new THREE.Euler(rx, ry, rz, 'XYZ');
  const originRotation = new THREE.Matrix4().makeRotationFromEuler(originEuler);
  matrix.multiply(originRotation);

  // 3. Apply joint rotation around its axis
  // Axis is given in the joint frame (after origin transform)
  // But we can just apply the rotation after the origin transform in the same space
  // Actually, standard URDF: axis is in the *joint frame* (which is origin frame).
  // So we apply rotation *after* origin rotation? No, origin rotation moves from parent to joint frame.
  // Then joint rotation is around the axis in that joint frame.
  // Wait, THREE.Euler applies rotation. If we multiply originRotation, the local axes are now aligned with the joint frame.
  // So we can just create a rotation matrix around the given axis and multiply it.
  const [ax, ay, az] = joint.axis;
  const axisVector = new THREE.Vector3(ax, ay, az).normalize();
  const jointRotation = new THREE.Matrix4().makeRotationAxis(axisVector, jointValue);
  matrix.multiply(jointRotation);

  return matrix;
}
