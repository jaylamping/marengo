// Vitest test for the URDF parser.
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { parseUrdfXml } from '../parse-urdf';

describe('parseUrdfXml', () => {
  const sampleUrdf = `<?xml version="1.0"?>
<robot name="test_robot">
  <link name="base_link">
    <inertial>
      <mass value="1.0"/>
      <inertia ixx="0.1" ixy="0" ixz="0" iyy="0.1" iyz="0" izz="0.1"/>
    </inertial>
  </link>
  <link name="upper_link">
    <inertial>
      <mass value="0.5"/>
      <origin xyz="0 0 0.1" rpy="0 0 0"/>
    </inertial>
  </link>
  <joint name="test_joint" type="revolute">
    <parent link="base_link"/>
    <child link="upper_link"/>
    <origin xyz="0 0 0" rpy="0 0 0"/>
    <axis xyz="0 1 0"/>
    <limit lower="-1.5" upper="1.5" effort="10" velocity="2.0"/>
    <safety_controller soft_lower_limit="-1.4" soft_upper_limit="1.4" k_position="10" k_velocity="1"/>
  </joint>
  <joint name="fixed_joint" type="fixed">
    <parent link="upper_link"/>
    <child link="sensor_mount"/>
  </joint>
</robot>`;

  it('should parse robot name and root links', () => {
    const model = parseUrdfXml(sampleUrdf);
    expect(model.name).toBe('test_robot');
    expect(model.rootLinks).toEqual(['base_link']);
  });

  it('should parse joints and links correctly', () => {
    const model = parseUrdfXml(sampleUrdf);
    expect(model.joints.size).toBe(2);
    expect(model.links.size).toBe(2);

    const joint = model.getJoint('test_joint');
    expect(joint).toBeDefined();
    expect(joint?.type).toBe('revolute');
    expect(joint?.parent).toBe('base_link');
    expect(joint?.child).toBe('upper_link');
    expect(joint?.axis).toEqual([0, 1, 0]);
    expect(joint?.limit).toEqual({ lower: -1.5, upper: 1.5, effort: 10, velocity: 2.0 });
    expect(joint?.safety).toEqual({ softLower: -1.4, softUpper: 1.4, kPosition: 10, kVelocity: 1 });
  });

  it('should parse fixed joints without limits', () => {
    const model = parseUrdfXml(sampleUrdf);
    const fixedJoint = model.getJoint('fixed_joint');
    expect(fixedJoint).toBeDefined();
    expect(fixedJoint?.type).toBe('fixed');
    expect(fixedJoint?.limit).toBeNull();
    expect(fixedJoint?.child).toBe('sensor_mount');
  });

  it('should compute chain from root correctly', () => {
    const model = parseUrdfXml(sampleUrdf);
    const chain = model.getChainFromRoot('test_joint');
    expect(chain.length).toBe(1);
    expect(chain[0].name).toBe('test_joint');
  });

  it('should warn if revolute joint lacks limit', () => {
    const urdfNoLimit = `<?xml version="1.0"?>
<robot name="no_limit_robot">
  <link name="base_link"/>
  <link name="child_link"/>
  <joint name="bad_joint" type="revolute">
    <parent link="base_link"/>
    <child link="child_link"/>
    <axis xyz="0 0 1"/>
  </joint>
</robot>`;
    const consoleSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    parseUrdfXml(urdfNoLimit);
    expect(consoleSpy).toHaveBeenCalledWith(expect.stringContaining("Missing <limit>"));
    consoleSpy.mockRestore();
  });
});
