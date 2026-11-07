select array(select x from unnest(array[1, 2], array[1, 2, 3]) x(a, b)) as arr
