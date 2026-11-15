create macro z_inner(x) as x + 1; create macro a_outer(x) as x.z_inner() > 0;
