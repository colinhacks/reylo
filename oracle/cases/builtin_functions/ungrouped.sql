select any_value(n) as some_n, percentile_cont(0.5) within group (order by amount) as median, mode() within group (order by mood) as usual from events
