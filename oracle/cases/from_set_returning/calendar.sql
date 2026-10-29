select d::date as day, count(v.id) as visits from generate_series(:start::date, :stop::date, interval '1 day') d left join visits v on v.at::date = d::date group by d
