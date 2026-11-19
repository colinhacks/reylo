select random() as unbounded, random(lo, lo) as bounded, random(hi, hi) as nullable_bound, json_build_object('r', random(hi, hi)) as built from bounds
