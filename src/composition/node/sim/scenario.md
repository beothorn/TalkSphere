# Simulation scenario

A `Scenario` holds the seed, the policy `Params` under test, the number and size of hosted items, the horizon and step, the
honest fleet, the issuance budget, the float each holder grants for maintenance and the prices of a check and a renewal.

The honest fleet: client count, mean seconds online and offline (exponentially distributed), mean seconds between offers to fulfil
the wanted terms, probability per day of losing everything stored (a wiped or replaced phone) and the probability that a connected
client does not answer a check in time. The defaults describe a deliberately unreliable fleet of 40 phones over one simulated week
(about 4 hours online, 8 hours offline, 1 % wipe per day, 5 % silent) hosting 20 items with a target of 3.

`validate` refuses zero sizes, a step longer than the horizon, probabilities outside `[0, 1]`, nonpositive durations, a policy
without wanted terms (replicas are placed through them) and invalid parameters.
