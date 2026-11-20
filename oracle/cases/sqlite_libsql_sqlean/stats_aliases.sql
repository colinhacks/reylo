select stats_median(score) as a, stats_perc(score, 25) as b, stats_p99(score) as c, stats_stddev(score) as d, stats_var_samp(score) as e from people
