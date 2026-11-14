select t.label, i.code from tags t join items i on i.id = t.item_id where t.item_id = :item_id
