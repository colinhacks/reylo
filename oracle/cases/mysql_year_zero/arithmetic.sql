select last_day(d) as l, cast(dt as date) as c, dt - interval 1 hour as before_dt, d + interval 1 day as next from early
