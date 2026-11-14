select v.arr from (values (array(select c from (select 1 as n) c)), (null)) v(arr)
