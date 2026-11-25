#!/usr/bin/env python3
"""Run fixed examples and retain independent PCG/Viper evidence."""
from pathlib import Path
import os,subprocess,shutil,json,hashlib,time
ROOT=Path(__file__).resolve().parent
PR=Path('/home/shenghao/GitHub/prusti')
CASES={
'reborrow':'prusti-tests/tests/verify/pass/borrows/reborrow_mut.rs',
'mutable':'prusti-tests/tests/verify/pass/quick/mut-borrows.rs',
'shared':'prusti-tests/tests/verify/pass/borrows/reborrow_shared.rs',
'arrays':'prusti-tests/tests/verify/pass/arrays/arrays_slices.rs',
'loop_inv':'prusti-tests/tests/verify/pass/loop-invs/simple-inv.rs',
'loop_borrow':'pcg/test-files/037_demo_loop.rs',
'branch':None,
'branch_only':None,
}
CUSTOM='''use prusti_contracts::*;

pub fn branch(flag: bool) {
    let mut a = 1;
    let mut b = 2;
    let r = if flag { &mut a } else { &mut b };
    *r = 9;
    if flag { assert!(a == 9); } else { assert!(b == 9); }
}

pub fn loop_reborrow(mut again: bool) {
    let mut x = 1;
    let mut r = &mut x;
    while again {
        r = &mut *r;
        again = false;
    }
    *r = 9;
    assert!(x == 9);
}
'''
records=[]
for name,source in CASES.items():
 out=ROOT/name
 if out.exists():
  records.append(json.loads((out/'run.json').read_text()));continue
 out.mkdir()
 src=out/(name+'.rs')
 if source:shutil.copy2(PR/source,src)
 else:src.write_text(CUSTOM.split('pub fn loop_reborrow')[0] if name=='branch_only' else CUSTOM)
 env=os.environ.copy();env.update(PCG_VISUALIZATION='true',PCG_VISUALIZATION_DATA_DIR=str(out/'data'),PRUSTI_DUMP_VIPER_PROGRAM='true',PRUSTI_LOG_DIR=str(out/'log'),PRUSTI_CHECK_OVERFLOWS='false')
 command=[str(PR/'target/release/prusti-rustc'),'--edition=2021','--crate-type=lib',str(src),'--out-dir',str(out)]
 start=time.time();print('RUN',name,flush=True)
 with (out/'run.log').open('w') as log:
  try:r=subprocess.run(command,cwd=out,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=180);status=r.returncode
  except subprocess.TimeoutExpired:status='timeout'
 record={'case':name,'upstream_source':source,'origin':'unchanged upstream' if source else 'authored demonstration; actually executed','sha256':hashlib.sha256(src.read_bytes()).hexdigest(),'command':command,'exit_code':status,'seconds':round(time.time()-start,2),'environment':{k:env[k] for k in ['PCG_VISUALIZATION','PCG_VISUALIZATION_DATA_DIR','PRUSTI_DUMP_VIPER_PROGRAM','PRUSTI_LOG_DIR','PRUSTI_CHECK_OVERFLOWS','JAVA_HOME','VIPER_HOME','Z3_EXE']},'prusti_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=PR,text=True).strip(),'pcg_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=PR/'pcg',text=True).strip()}
 (out/'run.json').write_text(json.dumps(record,indent=2)+'\n');records.append(record)
 (ROOT/'runs.json').write_text(json.dumps(records,indent=2)+'\n');print('DONE',name,status,flush=True)
