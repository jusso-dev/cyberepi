# Reproduction numbers

For an exponential infectious period that competes with transmission, the probability that `i` infects still-susceptible neighbour `j` before leaving the infectious compartment is `λ / (λ + γ)`. If detection leads into a second transmitting compartment, there is a further chance during that sojourn. Summing over neighbours is the expected secondary-case count, by linearity, whether or not the edges are independent.

R₀ is the mean of those counts over entities that start susceptible.

The invasion threshold is the leading eigenvalue of the operator that maps "who is infectious this generation" to "who they infect next". It can be much larger than the mean when a few hubs dominate. That is a structural fact about the contact graph, not a claim that every random spark becomes an epidemic. Monte Carlo extinction rates show how often a random index case fails to reach those hubs.

Rₜ recomputes the same expectation using only neighbours that are susceptible at that moment, averaged over entities that are currently infectious or detected. Samples are hourly by default.

Superspreader score blends expected secondary cases, observed secondary cases across runs, PageRank, degree, and privilege. Betweenness is approximated from a sample of sources when the population is at most 2,500 entities, because exact betweenness does not belong on a million-node homelab run. The explanation shown in the report says which of those terms were high.
