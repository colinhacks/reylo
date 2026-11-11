insert into replies values (5, 1, 'q') returning array(select c from (select old.body as ob, new.body as nb) c) as arr
