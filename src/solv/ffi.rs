use super::{PackageAction, PackageMeta};
use anyhow::{anyhow, Result};
use hex::encode;
use libc::{c_char, c_int};
use libsolv_sys::ffi;
use std::{convert::TryInto, ffi::CStr, os::unix::ffi::OsStrExt, slice};
use std::{ffi::CString, path::Path, ptr::null_mut};

pub const SELECTION_NAME: c_int = 1 << 0;
pub const SELECTION_FLAT: c_int = 1 << 10;

pub const SOLVER_FLAG_BEST_OBEY_POLICY: c_int = 12;

pub struct Pool {
    pool: *mut ffi::Pool,
}

macro_rules! cstr {
    ($s:expr) => {
        CString::new($s)?.as_ptr() as *const c_char
    };
}

macro_rules! to_string {
    ($s:ident, $v:expr) => {{
        let r = ffi::solvable_lookup_str($s, $v as i32);
        if r.is_null() {
            String::new()
        } else {
            CStr::from_ptr(r).to_string_lossy().to_string()
        }
    }};
}

fn change_to_action(change: ffi::Id) -> Result<PackageAction> {
    match change as u32 {
        ffi::SOLVER_TRANSACTION_INSTALL => Ok(PackageAction::Install(false)),
        ffi::SOLVER_TRANSACTION_DOWNGRADE => Ok(PackageAction::Downgrade),
        ffi::SOLVER_TRANSACTION_UPGRADE => Ok(PackageAction::Upgrade),
        ffi::SOLVER_TRANSACTION_REINSTALL => Ok(PackageAction::Install(true)),
        ffi::SOLVER_TRANSACTION_ERASE => Ok(PackageAction::Erase),
        ffi::SOLVER_TRANSACTION_IGNORE => Ok(PackageAction::Noop),
        _ => Err(anyhow!("Unknown action: {}", change)),
    }
}

#[inline]
fn solvable_to_meta(
    t: *mut ffi::Transaction,
    s: *mut ffi::Solvable,
    p: ffi::Id,
) -> Result<PackageMeta> {
    let mut sum_type: ffi::Id = 0;
    let checksum_ref = unsafe {
        ffi::solvable_lookup_bin_checksum(
            s,
            ffi::solv_knownid_SOLVABLE_CHECKSUM as i32,
            &mut sum_type,
        )
    };
    let checksum: &[u8];
    if sum_type == 0 {
        checksum = &[];
    } else if sum_type != (ffi::solv_knownid_REPOKEY_TYPE_SHA256 as i32) {
        return Err(anyhow!("Unsupported checksum type: {}", sum_type));
    } else {
        checksum = unsafe { slice::from_raw_parts(checksum_ref, 32) };
    }
    let name = unsafe { to_string!(s, ffi::solv_knownid_SOLVABLE_NAME) };
    let version = unsafe { to_string!(s, ffi::solv_knownid_SOLVABLE_EVR) };
    let path = unsafe { to_string!(s, ffi::solv_knownid_SOLVABLE_MEDIADIR) };
    let filename = unsafe { to_string!(s, ffi::solv_knownid_SOLVABLE_MEDIAFILE) };
    let change_type = unsafe {
        ffi::transaction_type(
            t,
            p,
            (ffi::SOLVER_TRANSACTION_SHOW_ACTIVE | ffi::SOLVER_TRANSACTION_CHANGE_IS_REINSTALL)
                as c_int,
        )
    };

    Ok(PackageMeta {
        name: name,
        version: version,
        sha256: encode(checksum),
        path: path + "/" + &filename,
        action: change_to_action(change_type)?,
    })
}

impl Pool {
    pub fn new() -> Pool {
        Pool {
            pool: unsafe { ffi::pool_create() },
        }
    }

    pub fn match_package(&self, name: &str, mut queue: Queue) -> Result<Queue> {
        if unsafe { (*self.pool).whatprovides.is_null() } {
            // we can't call createwhatprovides here because of how libsolv manages internal states
            return Err(anyhow!(
                "internal error: `createwhatprovides` needs to be called first."
            ));
        }
        let ret = unsafe {
            ffi::selection_make(
                self.pool,
                &mut queue.queue,
                cstr!(name),
                SELECTION_NAME | SELECTION_FLAT,
            )
        };
        if ret < 1 {
            return Err(anyhow!("Error matching the package: {}", name));
        }

        Ok(queue)
    }

    pub fn createwhatprovides(&mut self) {
        unsafe { ffi::pool_createwhatprovides(self.pool) }
    }

    pub fn set_installed(&mut self, repo: &Repo) {
        unsafe { ffi::pool_set_installed(self.pool, repo.repo) }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        unsafe { ffi::pool_free(self.pool) }
    }
}

pub struct Repo {
    repo: *mut ffi::Repo,
}

impl Repo {
    pub fn new(pool: &Pool, name: &str) -> Result<Repo> {
        let name = CString::new(name)?;
        Ok(Repo {
            repo: unsafe { ffi::repo_create(pool.pool, name.as_ptr()) },
        })
    }

    pub fn add_debpackages(&mut self, path: &Path) -> Result<()> {
        let mut path_buf = path.as_os_str().as_bytes().to_owned();
        path_buf.push(0);
        let fp = unsafe { libc::fopen(path_buf.as_ptr() as *const c_char, cstr!("rb")) };
        if fp.is_null() {
            return Err(anyhow!("Failed to open '{}'", path.display()));
        }
        let result = unsafe { ffi::repo_add_debpackages(self.repo, fp as *mut ffi::_IO_FILE, 0) };
        unsafe { libc::fclose(fp) };
        if result != 0 {
            return Err(anyhow!("Failed to add packages: {}", result));
        }

        Ok(())
    }

    pub fn add_debdb(&mut self) -> Result<()> {
        let result = unsafe { ffi::repo_add_debdb(self.repo, 0) };
        if result != 0 {
            return Err(anyhow!("Failed to add debdb: {}", result));
        }

        Ok(())
    }
}

pub struct Queue {
    queue: ffi::Queue,
}

impl Queue {
    pub fn new() -> Queue {
        Queue {
            queue: ffi::Queue {
                elements: null_mut(),
                count: 0,
                alloc: null_mut(),
                left: 0,
            },
        }
    }

    pub fn mark_all_as(&mut self, flags: c_int) {
        for item in (0..self.queue.count).step_by(2) {
            unsafe {
                let addr = self.queue.elements.offset(item.try_into().unwrap());
                (*addr) |= flags;
            }
        }
    }

    pub fn push2(&mut self, a: c_int, b: c_int) {
        self.push(a);
        self.push(b);
    }

    pub fn push(&mut self, item: c_int) {
        if self.queue.left < 1 {
            unsafe { ffi::queue_alloc_one(&mut self.queue) }
        }
        self.queue.count += 1;
        unsafe {
            let elem = self.queue.elements.offset(self.queue.count as isize);
            (*elem) = item;
        }
        self.queue.left -= 1;
    }

    pub fn extend(&mut self, q: &Queue) {
        unsafe {
            ffi::queue_insertn(
                &mut self.queue,
                self.queue.count,
                q.queue.count,
                q.queue.elements,
            )
        }
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        unsafe { ffi::queue_free(&mut self.queue) }
    }
}

pub struct Transaction {
    t: *mut ffi::Transaction,
}

impl Transaction {
    pub fn get_size_change(&self) -> i64 {
        unsafe { ffi::transaction_calc_installsizechange(self.t) }
    }

    pub fn order(&self, flags: c_int) {
        unsafe { ffi::transaction_order(self.t, flags) }
    }

    pub fn create_metadata(&self) -> Result<Vec<PackageMeta>> {
        let mut results = Vec::new();
        unsafe {
            let steps = (*self.t).steps.elements;
            for i in 0..((*self.t).steps.count) {
                let p = *steps.offset(i as isize);
                let pool = (*self.t).pool;
                results.push(solvable_to_meta(
                    self.t,
                    (*pool).solvables.offset(p as isize),
                    p,
                )?);
            }
        }

        Ok(results)
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        unsafe { ffi::transaction_free(self.t) }
    }
}

pub struct Solver {
    solver: *mut ffi::Solver,
}

impl Solver {
    pub fn new(pool: &Pool) -> Solver {
        Solver {
            solver: unsafe { ffi::solver_create(pool.pool) },
        }
    }

    pub fn set_flag(&mut self, flag: c_int, value: c_int) -> Result<()> {
        let result = unsafe { ffi::solver_set_flag(self.solver, flag, value) };
        if result != 0 {
            return Err(anyhow!("set_flag failed: {}", result));
        }

        Ok(())
    }

    pub fn create_transaction(&mut self) -> Result<Transaction> {
        let t = unsafe { ffi::solver_create_transaction(self.solver) };
        if t.is_null() {
            return Err(anyhow!("Failed to create transaction"));
        }

        Ok(Transaction { t })
    }

    pub fn solve(&self, queue: &mut Queue) -> Result<()> {
        let result = unsafe { ffi::solver_solve(self.solver, &mut queue.queue) };
        if result != 0 {
            let problem = self.get_problems();
            match problem {
                Ok(problems) => return Err(anyhow!("Issues found: \n{}", problems.join("\n"))),
                Err(_) => return Err(anyhow!("Solve failed: {}", result)),
            }
        }

        Ok(())
    }

    fn get_problems(&self) -> Result<Vec<String>> {
        let mut problems = Vec::new();
        let count = unsafe { ffi::solver_problem_count(self.solver) };
        for i in 1..=count {
            let problem = unsafe { ffi::solver_problem2str(self.solver, i as c_int) };
            if problem.is_null() {
                return Err(anyhow!("problem2str failed: {}", i));
            }
            problems.push(unsafe { CStr::from_ptr(problem).to_string_lossy().to_string() });
        }

        Ok(problems)
    }
}

impl Drop for Solver {
    fn drop(&mut self) {
        unsafe { ffi::solver_free(self.solver) }
    }
}
