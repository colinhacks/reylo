select jsonb_build_object('sd', stddev(p.n), 'vs', var_samp(p.n), 'c', corr(p.n, p.n), 'sl', regr_slope(p.n, p.n)) as j from posts p group by p.id
