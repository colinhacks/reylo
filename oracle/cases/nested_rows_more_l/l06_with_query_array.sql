with w as (select array(select c from replies c) as arr) select w.arr from holder2, w
