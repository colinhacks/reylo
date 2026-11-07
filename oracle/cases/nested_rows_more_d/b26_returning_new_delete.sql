delete from replies where id = 1 returning array(select c from (select new.body as nb, old.body as ob) c) as arr
