export type JointSpec = {
  name: string;
  type: 'revolute' | 'continuous' | 'prismatic' | 'fixed' | string;
  parent: string;
  child: string;
  axis: readonly [number, number, number];
  origin: { xyz: readonly [number, number, number]; rpy: readonly [number, number, number] };
  limit: { lower: number; upper: number; effort: number; velocity: number } | null;
  safety: { softLower: number; softUpper: number; kPosition: number; kVelocity: number } | null;
  mimic: { joint: string; multiplier: number; offset: number } | null;
};

export type LinkSpec = {
  name: string;
  inertial?: {
    mass: number;
    origin?: { xyz: readonly [number, number, number]; rpy: readonly [number, number, number] };
    inertia?: { ixx: number; ixy: number; ixz: number; iyy: number; iyz: number; izz: number };
  };
};

export type RobotModel = {
  name: string;
  joints: Map<string, JointSpec>;
  links: Map<string, LinkSpec>;
  rootLinks: readonly string[]; // Links that are not children of any joint
  getJoint: (name: string) => JointSpec | undefined;
  getChainFromRoot: (jointName: string) => JointSpec[];
};

function parseFloatArray(attrValue: string | null, length: number): readonly number[] {
  if (!attrValue) {
    return new Array(length).fill(0) as readonly number[];
  }
  return attrValue.trim().split(/\s+/).map((v) => parseFloat(v)) as readonly number[];
}

function getAttr(el: Element, attr: string): string | null {
  return el.getAttribute(attr);
}

function getRequiredAttr(el: Element, attr: string): string {
  const val = getAttr(el, attr);
  if (!val) throw new Error(`Missing required attribute '${attr}' on <${el.tagName.toLowerCase()}>`);
  return val;
}

export function parseUrdfXml(xml: string): RobotModel {
  if (typeof DOMParser === 'undefined') {
    throw new Error('DOMParser is not available. This must run in a browser environment.');
  }

  const parser = new DOMParser();
  const doc = parser.parseFromString(xml, 'text/xml');

  if (doc.querySelector('parsererror')) {
    const errorEl = doc.querySelector('parsererror');
    throw new Error(`Failed to parse URDF XML: ${errorEl?.textContent}`);
  }

  const robotEl = doc.querySelector('robot');
  if (!robotEl) {
    throw new Error('URDF must have a <robot> root element.');
  }

  const name = getRequiredAttr(robotEl, 'name');
  const joints = new Map<string, JointSpec>();
  const links = new Map<string, LinkSpec>();
  const childLinks = new Set<string>();

  // Parse links
  const linkEls = Array.from(robotEl.querySelectorAll('link'));
  for (const linkEl of linkEls) {
    const linkName = getRequiredAttr(linkEl, 'name');
    const linkSpec: LinkSpec = { name: linkName };

    const inertialEl = linkEl.querySelector(':scope > inertial');
    if (inertialEl) {
      const massEl = inertialEl.querySelector('mass');
      const massVal = massEl ? parseFloat(getRequiredAttr(massEl, 'value')) : undefined;

      const originEl = inertialEl.querySelector('origin');
      const originSpec = originEl
        ? {
            xyz: parseFloatArray(getAttr(originEl, 'xyz'), 3) as readonly [number, number, number],
            rpy: parseFloatArray(getAttr(originEl, 'rpy'), 3) as readonly [number, number, number],
          }
        : undefined;

      const inertiaEl = inertialEl.querySelector('inertia');
      const inertiaSpec = inertiaEl
        ? {
            ixx: parseFloat(getRequiredAttr(inertiaEl, 'ixx')),
            ixy: parseFloat(getAttr(inertiaEl, 'ixy') ?? '0'),
            ixz: parseFloat(getAttr(inertiaEl, 'ixz') ?? '0'),
            iyy: parseFloat(getRequiredAttr(inertiaEl, 'iyy')),
            iyz: parseFloat(getAttr(inertiaEl, 'iyz') ?? '0'),
            izz: parseFloat(getRequiredAttr(inertiaEl, 'izz')),
          }
        : undefined;

      if (massVal !== undefined) {
        linkSpec.inertial = { mass: massVal, origin: originSpec, inertia: inertiaSpec };
      }
    }

    links.set(linkName, linkSpec);
  }

  // Parse joints
  const jointEls = Array.from(robotEl.querySelectorAll('joint'));
  for (const jointEl of jointEls) {
    const jointName = getRequiredAttr(jointEl, 'name');
    const jointType = getRequiredAttr(jointEl, 'type');

    const parentEl = jointEl.querySelector('parent');
    const parentLink = parentEl ? getRequiredAttr(parentEl, 'link') : '';

    const childEl = jointEl.querySelector('child');
    const childLink = childEl ? getRequiredAttr(childEl, 'link') : '';
    childLinks.add(childLink);

    const axisEl = jointEl.querySelector('axis');
    const axis = axisEl
      ? (parseFloatArray(axisEl.getAttribute('xyz'), 3) as readonly [number, number, number])
      : ([0, 0, 0] as const);

    const originEl = jointEl.querySelector('origin');
    const origin = originEl
      ? {
          xyz: parseFloatArray(getAttr(originEl, 'xyz'), 3) as readonly [number, number, number],
          rpy: parseFloatArray(getAttr(originEl, 'rpy'), 3) as readonly [number, number, number],
        }
      : { xyz: [0, 0, 0] as const, rpy: [0, 0, 0] as const };

    const limitEl = jointEl.querySelector('limit');
    let limit: JointSpec['limit'] = null;
    if (limitEl) {
      limit = {
        lower: parseFloat(limitEl.getAttribute('lower') ?? '-Infinity'),
        upper: parseFloat(limitEl.getAttribute('upper') ?? 'Infinity'),
        effort: parseFloat(limitEl.getAttribute('effort') ?? '0'),
        velocity: parseFloat(limitEl.getAttribute('velocity') ?? '0'),
      };
    } else if (jointType === 'revolute' || jointType === 'prismatic') {
      console.warn(`[urdf] Missing <limit> on ${(jointType || 'revolute')} joint '${jointName}'`);
    }

    const safetyEl = jointEl.querySelector('safety_controller');
    let safety: JointSpec['safety'] = null;
    if (safetyEl) {
      safety = {
        softLower: parseFloat(safetyEl.getAttribute('soft_lower_limit') ?? '0'),
        softUpper: parseFloat(safetyEl.getAttribute('soft_upper_limit') ?? '0'),
        kPosition: parseFloat(safetyEl.getAttribute('k_position') ?? '0'),
        kVelocity: parseFloat(safetyEl.getAttribute('k_velocity') ?? '0'),
      };
    }

    const mimicEl = jointEl.querySelector('mimic');
    let mimic: JointSpec['mimic'] = null;
    if (mimicEl) {
      mimic = {
        joint: mimicEl.getAttribute('joint') || '',
        multiplier: parseFloat(mimicEl.getAttribute('multiplier') || '1'),
        offset: parseFloat(mimicEl.getAttribute('offset') || '0'),
      };
    }

    joints.set(jointName, {
      name: jointName,
      type: jointType,
      parent: parentLink,
      child: childLink,
      axis,
      origin,
      limit,
      safety,
      mimic,
    });
  }

  const rootLinks = Array.from(links.keys()).filter((name) => !childLinks.has(name));

  return {
    name,
    joints,
    links,
    rootLinks: Object.freeze(rootLinks) as readonly string[],
    getJoint: (name: string) => joints.get(name),
    getChainFromRoot: (jointName: string) => {
      const chain: JointSpec[] = [];
      let currentJoint = joints.get(jointName);
      while (currentJoint) {
        chain.unshift(currentJoint);
        let parentJoint: JointSpec | undefined;
        for (const j of joints.values()) {
          if (j.child === currentJoint.parent) {
            parentJoint = j;
            break;
          }
        }
        currentJoint = parentJoint;
      }
      return chain;
    },
  };
}

// Browser-specific helper with fallback
let cachedModel: RobotModel | null = null;
export function getRobotModel(xml: string): RobotModel {
  if (!cachedModel) {
    cachedModel = parseUrdfXml(xml);
  }
  return cachedModel;
}
