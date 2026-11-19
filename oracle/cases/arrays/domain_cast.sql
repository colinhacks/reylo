select array[x::int[]] as cast_one, array[x || 5] as appended, array[x[1:1]] as sliced, jsonb_build_object('a', array[x::int[]]) as doc from lists
