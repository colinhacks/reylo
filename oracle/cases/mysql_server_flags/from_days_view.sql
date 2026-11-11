-- A view's FROM_DAYS is NULL past 9999-12-31 too, though the statement never calls it.
select id, d, far from far_days
