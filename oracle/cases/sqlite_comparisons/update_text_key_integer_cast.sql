update t set id = id where code = cast(:c as integer) returning id
