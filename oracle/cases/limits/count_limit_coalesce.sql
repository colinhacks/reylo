select count(*) from bar limit coalesce(:n::nullable(int), 0)
