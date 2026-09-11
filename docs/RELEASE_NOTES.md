# Remover 0.4.6

The fly now renders in 3D. Its body, compound eyes, paired wings, antennae, and six legs share one perspective camera and depth buffer. Near and far surfaces have separate positions and shading.

Brain clusters and fly behavior now follow the current app state: collection, activity checks, removal, reconciliation, cooldown, paused, disconnected, blocked, or idle. Both panels show the same state. Confirmed removals trigger a separate brief response. Completed batches appear idle, and blocked jobs stop animation updates.

The artwork visualizes app state, not measured neural activity. Cleanup rules, pacing, and the Chrome driver are unchanged.
