<p align="center">
  <img src="../../docs/portraits/davout.jpg" alt="Louis-Nicolas Davout" width="420"/>
</p>

# davout

Davout is the safety layer.

It sits between [Berthier](../berthier/) and hardware: joint limits (URDF + kinematics docs), rate limits, fault reactions. Nothing reaches the motors without passing Davout.
