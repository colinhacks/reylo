create type pair as (a int, b int);
revoke all on type pair from public;
create type kept as (a int);
