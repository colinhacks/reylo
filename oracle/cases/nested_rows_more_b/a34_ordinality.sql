select array(select x from unnest(array[1, null]) with ordinality x(v, n)) as arr
