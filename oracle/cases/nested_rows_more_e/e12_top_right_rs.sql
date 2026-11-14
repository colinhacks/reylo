select l.rs from (select array(select r from replies r) as rs) l right join comments x on false
