drop index t_norm;
drop macro norm;
create macro norm (s, keep) as (case  when (keep) then (s) else lower(s) end);
create index t_norm on t((norm(email, false)));
