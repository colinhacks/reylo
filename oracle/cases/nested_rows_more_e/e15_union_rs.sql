select u.rs from (select array(select r from replies r) as rs union all select null) u
