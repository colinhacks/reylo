-- A FROM_DAYS outside the select list and the FROM makes no result column.
select id, n from t where from_days(n) > '2000-01-01' group by id, n having from_days(max(n)) is not null order by from_days(n)
