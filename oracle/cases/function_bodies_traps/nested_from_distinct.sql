select distinct (select l.v from lit(pick(c.id)) l) as v from comments c
