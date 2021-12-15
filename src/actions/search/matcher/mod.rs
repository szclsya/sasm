fn search_simple(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < needle.len() {
        return None;
    }

    for offset in 0..=(haystack.len() - needle.len()) {
        if haystack[offset..(offset + needle.len())] == *needle {
            return Some(offset);
        }
    }

    None
}

/// Adapted from https://github.com/ashvardanian/CppBenchSubstrSearch/blob/444d9acc627fa23ed2f21401ffa45d2b919a66a8/substr_search.hpp
fn search_rk_generic(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() < 5 {
        return search_simple(haystack, needle);
    }
    let h_end = haystack.len();
    let n_prefix: u32 = unsafe { *(std::mem::transmute::<_, *const u32>(needle.as_ptr())) };
    let n_suffix_len = needle.len() - 4;
    for h_offset in 0..h_end {
        if n_prefix
            == unsafe { *(std::mem::transmute::<_, *const u32>(haystack[h_offset..].as_ptr())) }
        {
            if haystack[(h_offset + 4)..(h_offset + 4 + n_suffix_len)] == needle[4..] {
                return Some(h_offset);
            }
        }
    }

    None
}

/// Adapted from https://github.com/ashvardanian/CppBenchSubstrSearch/blob/444d9acc627fa23ed2f21401ffa45d2b919a66a8/substr_search.hpp
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn search_avx2(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    use std::arch::x86_64::*;

    if needle.len() < 5 {
        return search_simple(haystack, needle);
    }

    let mut h_ptr = haystack.as_ptr() as *const u8;
    let h_end = /* unsafe */ { h_ptr.add(haystack.len() - needle.len()) };
    /* unsafe */
    {
        let needle_4b: *const i32 = std::mem::transmute(needle.as_ptr());
        let n_prefix = _mm256_set1_epi32(*needle_4b);
        while h_ptr.add(32) <= h_end {
            let h0 = _mm256_loadu_si256(h_ptr as *const _);
            let mask0 = _mm256_movemask_epi8(_mm256_cmpeq_epi32(h0, n_prefix));
            let h1 = _mm256_loadu_si256(h_ptr.offset(1) as *const _);
            let mask1 = _mm256_movemask_epi8(_mm256_cmpeq_epi32(h1, n_prefix));
            let h2 = _mm256_loadu_si256(h_ptr.offset(2) as *const _);
            let mask2 = _mm256_movemask_epi8(_mm256_cmpeq_epi32(h2, n_prefix));
            let h3 = _mm256_loadu_si256(h_ptr.offset(3) as *const _);
            let mask3 = _mm256_movemask_epi8(_mm256_cmpeq_epi32(h3, n_prefix));

            if (mask0 | mask1 | mask2 | mask3) != 0 {
                let current = std::slice::from_raw_parts(h_ptr, 32 + needle.len());
                for i in 0..32 {
                    if current[i..(i + needle.len())] == *needle {
                        let rest_offset = h_ptr.offset_from(haystack.as_ptr()) as usize;
                        return Some(i + rest_offset);
                    }
                }
            }

            h_ptr = h_ptr.add(32);
        }
        // last < 35 bytes
        let rest_offset = h_ptr.offset_from(haystack.as_ptr()) as usize;
        let rest = search_rk_generic(&haystack[rest_offset..], needle);
        return rest.and_then(|x| Some(rest_offset + x));
    }
}

pub fn search_rk_fast(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { search_avx2(haystack, needle) };
        }
    }
    return search_rk_generic(haystack, needle);
}
