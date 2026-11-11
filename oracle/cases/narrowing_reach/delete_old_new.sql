delete from orders where note = :n returning old.note as before, new.note as after
