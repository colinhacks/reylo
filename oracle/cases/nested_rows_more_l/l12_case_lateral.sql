select l.arr from holder2, lateral (select array(select case when true then c end from replies c) as arr) l
