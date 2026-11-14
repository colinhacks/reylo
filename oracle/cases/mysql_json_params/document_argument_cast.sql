update profiles set doc = json_merge_patch(doc, :patch::json) where id = :id
