select d from generate_series('2026-01-01'::timestamptz, '2026-01-03'::timestamptz, interval '1 day') d left join visits v on v.at::date = d::date
