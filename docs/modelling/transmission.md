# Transmission

The conceptual daily transmission product is:

```text
pathogen infectiousness
× channel weight
× source infectiousness
× target susceptibility
× edge modifier
× contact frequency
× privilege modifier
× environmental modifier
× control modifier
```

Contact frequency is an opportunity rate and may exceed 1, so the product is not automatically a probability. The default law, `clamped-product`, clamps the product into `[0, 1]` and treats that as the probability of transmission across one day. The Gillespie hazard is `λ = -ln(1 - p)`, capped at 24 per day when `p = 1`.

`exponential-saturation` uses `p = 1 - exp(-raw)` instead, so large products saturate smoothly.

Edge modifier folds trust into the stored transmission modifier. Environmental modifier folds segmentation, patch level, and exposure. Privilege modifier folds source privilege and access strength. All three are bounded. They are illustrative.

Channel weights let a pathogen prefer identity edges, email, cloud, or supply-chain relationships without encoding a technique.

Control coverage is a deterministic function of the population seed, the entity id, and the control. An 80% MFA scenario covers the same entities on every replay.
