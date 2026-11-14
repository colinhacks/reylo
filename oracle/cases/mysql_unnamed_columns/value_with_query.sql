with n as (select count(*) as c from users) select c + 1 from n
