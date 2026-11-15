drop index t_e;
drop macro norm;
create macro norm (x) as (upper(x));
create index t_e on t((norm(e)));
comment on index t_e is 'by e';
