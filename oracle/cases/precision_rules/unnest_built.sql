select unnest(array[[1, 2], [3, 4]]) as u, unnest(array[1, null]) as n
