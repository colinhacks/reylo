insert into profiles (id, doc) values (:id, :doc) on duplicate key update doc = :again
