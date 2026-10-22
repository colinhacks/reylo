select array[id] @> array[1] as c, array[id] && array[2] as o, array[id] <@ array[1, 2] as d, tags @> array['x'] as t from member
