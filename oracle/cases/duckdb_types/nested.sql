-- Nested columns declared NOT NULL, which the oracle seeds with values of their own.
select id, shape, shape.a as a, pairs, choice, fixed, fixed[1] as first from nested
