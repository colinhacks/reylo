create type pair as (a int, b int);
create type kept as (a int);
revoke usage on type kept from public;
