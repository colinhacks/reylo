SELECT id > 1 ? abs(:n::Int32) : 0 AS v, id > 1 ? name :name::String AS w FROM users
