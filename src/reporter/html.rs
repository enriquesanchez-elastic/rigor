//! HTML reporter: generates a self-contained interactive HTML dashboard
//!
//! Embeds analysis results as JSON and renders everything client-side
//! with vanilla JS for filtering, sorting, searching, and folder grouping.

use crate::analyzer::engine::AggregateStats;
use crate::analyzer::scoring::ScoreCalculator;
use crate::{AnalysisResult, Severity};
use serde::Serialize;

/// Escapes a string for embedding inside a JS string literal (backtick template)
fn escape_json_for_script(s: &str) -> String {
    // serde_json already escapes quotes/backslashes; we just need to ensure
    // no </script> can appear inside the block.
    s.replace("</script>", "<\\/script>")
}

/// Reporter that generates a self-contained HTML dashboard
pub struct HtmlReporter;

/// Lightweight per-issue struct for the JSON payload (avoids exposing internal types)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsIssue {
    rule: String,
    severity: String,
    message: String,
    line: usize,
    column: usize,
    suggestion: Option<String>,
}

/// Lightweight per-file struct for the JSON payload
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsFile {
    path: String,
    score: u8,
    grade: String,
    framework: String,
    test_type: String,
    total_tests: usize,
    total_assertions: usize,
    assertion_quality: u8,
    error_coverage: u8,
    boundary_conditions: u8,
    test_isolation: u8,
    input_variety: u8,
    ai_smells: u8,
    issues: Vec<JsIssue>,
    recommendations: Vec<String>,
}

impl HtmlReporter {
    pub fn new() -> Self {
        Self
    }

    /// Generate the full HTML report
    pub fn report(&self, results: &[AnalysisResult], stats: &AggregateStats) -> String {
        let js_files: Vec<JsFile> = results.iter().map(|r| self.to_js_file(r)).collect();
        let data_json = serde_json::to_string(&js_files).unwrap_or_else(|_| "[]".to_string());

        let mut html = String::with_capacity(32_768);
        html.push_str(Self::template_head());
        html.push_str("<script>const DATA=");
        html.push_str(&escape_json_for_script(&data_json));
        html.push_str(";const STATS={files:");
        html.push_str(&stats.files_analyzed.to_string());
        html.push_str(",avg:");
        html.push_str(&stats.average_score.value.to_string());
        html.push_str(",grade:\"");
        html.push_str(&stats.average_score.grade.to_string());
        html.push_str("\",tests:");
        html.push_str(&stats.total_tests.to_string());
        html.push_str(",issues:");
        html.push_str(&stats.total_issues.to_string());
        html.push_str("};</script>\n");
        html.push_str(Self::template_body());
        html.push_str(Self::template_script());
        html.push_str("</body>\n</html>");
        html
    }

    fn to_js_file(&self, r: &AnalysisResult) -> JsFile {
        let issues: Vec<JsIssue> = r
            .issues
            .iter()
            .map(|i| JsIssue {
                rule: i.rule.to_string(),
                severity: match i.severity {
                    Severity::Error => "error".into(),
                    Severity::Warning => "warning".into(),
                    Severity::Info => "info".into(),
                },
                message: i.message.clone(),
                line: i.location.line,
                column: i.location.column,
                suggestion: i.suggestion.clone(),
            })
            .collect();

        let recs = if r.score.value < 90 {
            ScoreCalculator::recommendations(&r.breakdown, &r.issues, r.score.grade)
                .into_iter()
                .take(5)
                .collect()
        } else {
            vec![]
        };

        JsFile {
            path: r.file_path.display().to_string(),
            score: r.score.value,
            grade: r.score.grade.to_string(),
            framework: r.framework.to_string(),
            test_type: r.test_type.to_string(),
            total_tests: r.stats.total_tests,
            total_assertions: r.stats.total_assertions,
            assertion_quality: r.breakdown.assertion_quality,
            error_coverage: r.breakdown.error_coverage,
            boundary_conditions: r.breakdown.boundary_conditions,
            test_isolation: r.breakdown.test_isolation,
            input_variety: r.breakdown.input_variety,
            ai_smells: r.breakdown.ai_smells,
            issues,
            recommendations: recs,
        }
    }

    // ─── HTML template pieces ────────────────────────────────────────────

    fn template_head() -> &'static str {
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Rigor – Test Quality Report</title>
<style>
:root{--bg:#0d0d11;--surface:#16161b;--surface2:#1e1e24;--border:#2a2a32;--text:#e4e4e7;--muted:#71717a;--green:#22c55e;--lime:#84cc16;--yellow:#eab308;--orange:#f97316;--red:#ef4444;--blue:#3b82f6;--purple:#a855f7;--cyan:#06b6d4;--radius:8px}
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Oxygen,sans-serif;background:var(--bg);color:var(--text);line-height:1.5;min-height:100vh}
::selection{background:var(--blue);color:#fff}

/* ── Layout ── */
.shell{display:grid;grid-template-columns:1fr 300px;grid-template-rows:auto auto 1fr;gap:0;min-height:100vh}
@media(max-width:960px){.shell{grid-template-columns:1fr;}}
header{grid-column:1/-1;padding:1.25rem 1.5rem;border-bottom:1px solid var(--border);display:flex;align-items:center;gap:1.5rem;flex-wrap:wrap}
header h1{font-size:1.125rem;font-weight:700;white-space:nowrap}
header .meta{font-size:.8125rem;color:var(--muted)}

/* ── Stats bar ── */
.stats-bar{grid-column:1/-1;display:flex;gap:0;border-bottom:1px solid var(--border);background:var(--surface)}
.stat{flex:1;padding:.875rem 1.25rem;border-right:1px solid var(--border);text-align:center}
.stat:last-child{border-right:none}
.stat .val{font-size:1.5rem;font-weight:700;display:block}
.stat .lbl{font-size:.75rem;color:var(--muted);text-transform:uppercase;letter-spacing:.5px}

/* ── Controls ── */
.controls{grid-column:1;padding:1rem 1.5rem;border-bottom:1px solid var(--border);display:flex;gap:.75rem;flex-wrap:wrap;align-items:center}
.search{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);padding:.5rem .75rem;color:var(--text);font-size:.8125rem;width:220px;outline:none;transition:border-color .15s}
.search:focus{border-color:var(--blue)}
.search::placeholder{color:var(--muted)}
.pill-group{display:flex;gap:2px;background:var(--surface);border-radius:var(--radius);padding:2px;border:1px solid var(--border)}
.pill{padding:.3rem .7rem;font-size:.75rem;font-weight:600;border-radius:6px;cursor:pointer;border:none;background:transparent;color:var(--muted);transition:all .15s}
.pill:hover{color:var(--text)}
.pill.active{background:var(--surface2);color:var(--text);box-shadow:0 1px 3px rgba(0,0,0,.3)}
.pill[data-grade="A"].active{color:var(--green)}
.pill[data-grade="B"].active{color:var(--lime)}
.pill[data-grade="C"].active{color:var(--yellow)}
.pill[data-grade="D"].active{color:var(--orange)}
.pill[data-grade="F"].active{color:var(--red)}
select.sort-sel{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);padding:.45rem .6rem;color:var(--text);font-size:.8125rem;outline:none;cursor:pointer}
.count-badge{font-size:.75rem;color:var(--muted);margin-left:auto;white-space:nowrap}

/* ── Main + Sidebar ── */
.main{grid-column:1;padding:1rem 1.5rem;overflow-y:auto}
.sidebar{grid-column:2;border-left:1px solid var(--border);padding:1rem 1.25rem;overflow-y:auto;background:var(--surface)}
@media(max-width:960px){
  .sidebar{grid-column:1;border-left:none;border-top:1px solid var(--border)}
  .controls{grid-column:1}
}

/* ── Sidebar sections ── */
.sb-section{margin-bottom:1.5rem}
.sb-section h3{font-size:.75rem;text-transform:uppercase;letter-spacing:.5px;color:var(--muted);margin-bottom:.5rem;padding-bottom:.375rem;border-bottom:1px solid var(--border)}
.sb-item{display:flex;justify-content:space-between;align-items:center;padding:.3rem 0;font-size:.8125rem}
.sb-item .name{color:var(--text);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:180px}
.sb-item .cnt{color:var(--muted);font-variant-numeric:tabular-nums;font-size:.75rem}
.sb-bar{height:3px;background:var(--border);border-radius:2px;margin-top:2px}
.sb-bar span{display:block;height:100%;border-radius:2px}

/* ── Grade distribution ── */
.distro{display:flex;gap:3px;align-items:flex-end;height:48px;margin-bottom:.75rem}
.distro-col{flex:1;display:flex;flex-direction:column;align-items:center;gap:2px}
.distro-col .bar{width:100%;border-radius:3px 3px 0 0;min-height:2px;transition:height .3s}
.distro-col .lbl{font-size:.625rem;color:var(--muted);font-weight:600}

/* ── Folder groups ── */
.folder{margin-bottom:.5rem}
.folder-hdr{display:flex;align-items:center;gap:.5rem;padding:.5rem .75rem;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);cursor:pointer;user-select:none;transition:background .15s}
.folder-hdr:hover{background:var(--surface2)}
.folder-hdr .chevron{font-size:.625rem;color:var(--muted);transition:transform .2s;width:12px}
.folder-hdr.open .chevron{transform:rotate(90deg)}
.folder-hdr .fname{font-size:.8125rem;font-weight:600;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.folder-hdr .badge{font-size:.6875rem;padding:.125rem .5rem;border-radius:10px;font-weight:600}
.folder-body{padding-left:1rem;overflow:hidden}
.folder-body.collapsed{display:none}

/* ── File rows ── */
.file-row{display:grid;grid-template-columns:minmax(0,1fr) 60px 40px 50px 50px;align-items:center;gap:.5rem;padding:.5rem .75rem;border-bottom:1px solid var(--border);cursor:pointer;transition:background .1s;font-size:.8125rem}
.file-row:hover{background:var(--surface2)}
.file-row:last-child{border-bottom:none}
.file-row .fpath{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--text)}
.file-row .fpath .dir{color:var(--muted)}
.score-pill{display:inline-flex;align-items:center;gap:4px;font-weight:600;font-variant-numeric:tabular-nums}
.grade-A,.grade-B{color:var(--green)}
.grade-C{color:var(--yellow)}
.grade-D{color:var(--orange)}
.grade-F{color:var(--red)}
.file-row .tests-col{color:var(--muted);font-variant-numeric:tabular-nums;text-align:right}
.file-row .issues-col{font-variant-numeric:tabular-nums;text-align:right}
.mini-bar{width:60px;height:6px;background:var(--border);border-radius:3px;overflow:hidden}
.mini-bar span{display:block;height:100%;border-radius:3px}

/* ── File detail panel ── */
.detail{display:none;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);margin:.5rem 0 .5rem 0;padding:1rem}
.detail.open{display:block}
.detail h4{font-size:.875rem;margin-bottom:.75rem}
.cat-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:.5rem;margin-bottom:1rem}
.cat-card{background:var(--surface2);border-radius:6px;padding:.5rem .75rem}
.cat-card .cat-name{font-size:.6875rem;color:var(--muted);text-transform:uppercase;letter-spacing:.3px}
.cat-card .cat-score{font-size:1rem;font-weight:700;margin-top:2px}
.cat-bar{height:4px;background:var(--border);border-radius:2px;margin-top:4px}
.cat-bar span{display:block;height:100%;border-radius:2px}
.issue-list{margin-top:.75rem}
.issue-item{font-size:.8125rem;padding:.35rem 0;border-bottom:1px solid var(--border);display:grid;grid-template-columns:auto 1fr auto;gap:.5rem;align-items:start}
.issue-item:last-child{border-bottom:none}
.issue-item .sev{font-size:.6875rem;font-weight:700;padding:.1rem .375rem;border-radius:4px;text-transform:uppercase;white-space:nowrap}
.sev-error{background:rgba(239,68,68,.15);color:var(--red)}
.sev-warning{background:rgba(234,179,8,.12);color:var(--yellow)}
.sev-info{background:rgba(59,130,246,.12);color:var(--blue)}
.issue-item .msg{color:var(--text)}
.issue-item .rule-tag{font-size:.6875rem;color:var(--muted);font-family:'SF Mono',Consolas,monospace;white-space:nowrap}
.issue-item .suggestion{font-size:.75rem;color:var(--muted);font-style:italic;grid-column:2/4;padding-top:2px}
.rec-list{margin-top:.75rem;padding-left:1rem}
.rec-list li{font-size:.8125rem;color:var(--muted);margin-bottom:.25rem}

/* ── Empty state ── */
.empty{text-align:center;padding:3rem 1rem;color:var(--muted);font-size:.875rem}

/* ── Utility ── */
.c-green{color:var(--green)}.c-lime{color:var(--lime)}.c-yellow{color:var(--yellow)}.c-orange{color:var(--orange)}.c-red{color:var(--red)}
.bg-green{background:var(--green)}.bg-lime{background:var(--lime)}.bg-yellow{background:var(--yellow)}.bg-orange{background:var(--orange)}.bg-red{background:var(--red)}
</style>
</head>
<body>
"##
    }

    fn template_body() -> &'static str {
        r##"<div class="shell">
  <header>
    <h1>Rigor</h1>
    <span class="meta" id="meta"></span>
  </header>
  <div class="stats-bar" id="stats-bar"></div>
  <div class="controls" id="controls-area">
    <input type="search" class="search" id="search" placeholder="Search files…" autocomplete="off">
    <div class="pill-group" id="grade-filter">
      <button class="pill active" data-grade="all">All</button>
      <button class="pill" data-grade="A">A</button>
      <button class="pill" data-grade="B">B</button>
      <button class="pill" data-grade="C">C</button>
      <button class="pill" data-grade="D">D</button>
      <button class="pill" data-grade="F">F</button>
    </div>
    <select class="sort-sel" id="sort-sel">
      <option value="score-asc">Score ↑ (worst first)</option>
      <option value="score-desc">Score ↓ (best first)</option>
      <option value="name">Name A→Z</option>
      <option value="issues">Most issues</option>
      <option value="tests">Most tests</option>
    </select>
    <span class="count-badge" id="count-badge"></span>
  </div>
  <div class="main" id="main"></div>
  <div class="sidebar" id="sidebar"></div>
</div>
"##
    }

    fn template_script() -> &'static str {
        r##"<script>
(function(){
"use strict";

/* ── helpers ── */
const $=s=>document.querySelector(s);
const $$=s=>[...document.querySelectorAll(s)];
const esc=s=>{const d=document.createElement('div');d.textContent=s;return d.innerHTML};
const gradeColor=g=>({A:'green',B:'lime',C:'yellow',D:'orange',F:'red'}[g]||'muted');
const barColor=s=>s>=80?'var(--green)':s>=60?'var(--yellow)':'var(--red)';

/* ── state ── */
let activeGrade='all';
let sortBy='score-asc';
let query='';
let openDetails=new Set();

/* ── render stats ── */
function renderStats(){
  const el=$('#stats-bar');
  const gc=gradeColor(STATS.grade);
  el.innerHTML=`
    <div class="stat"><span class="val">${STATS.files}</span><span class="lbl">Files</span></div>
    <div class="stat"><span class="val c-${gc}">${STATS.avg} <small>${STATS.grade}</small></span><span class="lbl">Avg score</span></div>
    <div class="stat"><span class="val">${STATS.tests}</span><span class="lbl">Tests</span></div>
    <div class="stat"><span class="val${STATS.issues>0?' c-red':''}">${STATS.issues}</span><span class="lbl">Issues</span></div>`;
  $('#meta').textContent=`Test Quality Report · ${new Date().toLocaleDateString()}`;
}

/* ── render sidebar ── */
function renderSidebar(){
  const sb=$('#sidebar');

  /* grade distribution */
  const dist={A:0,B:0,C:0,D:0,F:0};
  DATA.forEach(f=>dist[f.grade]++);
  const maxD=Math.max(...Object.values(dist),1);
  let distHtml='<div class="sb-section"><h3>Grade Distribution</h3><div class="distro">';
  for(const g of ['A','B','C','D','F']){
    const h=Math.round((dist[g]/maxD)*40);
    distHtml+=`<div class="distro-col"><div class="bar bg-${gradeColor(g)}" style="height:${h}px"></div><div class="lbl">${g}<br>${dist[g]}</div></div>`;
  }
  distHtml+='</div></div>';

  /* top rules */
  const ruleMap={};
  DATA.forEach(f=>f.issues.forEach(i=>{ruleMap[i.rule]=(ruleMap[i.rule]||0)+1}));
  const topRules=Object.entries(ruleMap).sort((a,b)=>b[1]-a[1]).slice(0,10);
  const maxR=topRules.length?topRules[0][1]:1;
  let rulesHtml='<div class="sb-section"><h3>Top Issues</h3>';
  for(const[rule,cnt] of topRules){
    const pct=Math.round(cnt/maxR*100);
    rulesHtml+=`<div class="sb-item"><span class="name" title="${esc(rule)}">${esc(rule)}</span><span class="cnt">${cnt}</span></div><div class="sb-bar"><span style="width:${pct}%;background:var(--blue)"></span></div>`;
  }
  if(!topRules.length) rulesHtml+='<div class="sb-item"><span class="name" style="color:var(--muted)">No issues found</span></div>';
  rulesHtml+='</div>';

  /* worst files */
  const worst=[...DATA].sort((a,b)=>a.score-b.score).slice(0,8);
  let worstHtml='<div class="sb-section"><h3>Needs Attention</h3>';
  for(const f of worst){
    const name=f.path.split('/').pop();
    const gc=gradeColor(f.grade);
    worstHtml+=`<div class="sb-item"><span class="name" title="${esc(f.path)}">${esc(name)}</span><span class="cnt c-${gc}">${f.score} ${f.grade}</span></div>`;
  }
  worstHtml+='</div>';

  /* category averages */
  const cats=['assertionQuality','errorCoverage','boundaryConditions','testIsolation','inputVariety','aiSmells'];
  const catLabels={assertionQuality:'Assertion Quality',errorCoverage:'Error Coverage',boundaryConditions:'Boundary Conditions',testIsolation:'Test Isolation',inputVariety:'Input Variety',aiSmells:'AI Smells'};
  let catHtml='<div class="sb-section"><h3>Category Averages</h3>';
  for(const c of cats){
    const avg=DATA.length?Math.round(DATA.reduce((s,f)=>s+f[c],0)/DATA.length):0;
    const pct=Math.round(avg/25*100);
    const col=avg>=20?'var(--green)':avg>=15?'var(--yellow)':'var(--red)';
    catHtml+=`<div class="sb-item"><span class="name">${catLabels[c]}</span><span class="cnt">${avg}/25</span></div><div class="sb-bar"><span style="width:${pct}%;background:${col}"></span></div>`;
  }
  catHtml+='</div>';

  sb.innerHTML=distHtml+worstHtml+rulesHtml+catHtml;
}

/* ── filter + sort ── */
function getVisible(){
  let files=DATA;
  if(activeGrade!=='all') files=files.filter(f=>f.grade===activeGrade);
  if(query){
    const q=query.toLowerCase();
    files=files.filter(f=>f.path.toLowerCase().includes(q));
  }
  switch(sortBy){
    case 'score-asc':files=[...files].sort((a,b)=>a.score-b.score);break;
    case 'score-desc':files=[...files].sort((a,b)=>b.score-a.score);break;
    case 'name':files=[...files].sort((a,b)=>a.path.localeCompare(b.path));break;
    case 'issues':files=[...files].sort((a,b)=>b.issues.length-a.issues.length);break;
    case 'tests':files=[...files].sort((a,b)=>b.totalTests-a.totalTests);break;
  }
  return files;
}

/* ── group by folder ── */
function groupByFolder(files){
  const map=new Map();
  for(const f of files){
    const parts=f.path.replace(/\\/g,'/').split('/');
    const name=parts.pop();
    const dir=parts.join('/')||'.';
    if(!map.has(dir)) map.set(dir,[]);
    map.get(dir).push({...f,fileName:name});
  }
  /* sort folders to match the active sort criteria */
  return [...map.entries()].sort((a,b)=>{
    const [,itemsA]=a,[,itemsB]=b;
    switch(sortBy){
      case 'score-asc':{
        const avgA=itemsA.reduce((s,f)=>s+f.score,0)/itemsA.length;
        const avgB=itemsB.reduce((s,f)=>s+f.score,0)/itemsB.length;
        return avgA-avgB;
      }
      case 'score-desc':{
        const avgA=itemsA.reduce((s,f)=>s+f.score,0)/itemsA.length;
        const avgB=itemsB.reduce((s,f)=>s+f.score,0)/itemsB.length;
        return avgB-avgA;
      }
      case 'name':return a[0].localeCompare(b[0]);
      case 'issues':{
        const iA=itemsA.reduce((s,f)=>s+f.issues.length,0);
        const iB=itemsB.reduce((s,f)=>s+f.issues.length,0);
        return iB-iA;
      }
      case 'tests':{
        const tA=itemsA.reduce((s,f)=>s+f.totalTests,0);
        const tB=itemsB.reduce((s,f)=>s+f.totalTests,0);
        return tB-tA;
      }
      default:return 0;
    }
  });
}

/* ── render file detail ── */
function renderDetail(f){
  const cats=[
    {key:'assertionQuality',name:'Assertion Quality'},
    {key:'errorCoverage',name:'Error Coverage'},
    {key:'boundaryConditions',name:'Boundary Conditions'},
    {key:'testIsolation',name:'Test Isolation'},
    {key:'inputVariety',name:'Input Variety'},
    {key:'aiSmells',name:'AI Smells'},
  ];
  let html='<div class="cat-grid">';
  for(const c of cats){
    const v=f[c.key];
    const pct=Math.round(v/25*100);
    const col=v>=20?'var(--green)':v>=15?'var(--yellow)':'var(--red)';
    html+=`<div class="cat-card"><div class="cat-name">${c.name}</div><div class="cat-score" style="color:${col}">${v}<small>/25</small></div><div class="cat-bar"><span style="width:${pct}%;background:${col}"></span></div></div>`;
  }
  html+='</div>';

  html+=`<div style="font-size:.75rem;color:var(--muted);margin-bottom:.75rem">${f.framework} · ${f.testType} · ${f.totalTests} tests · ${f.totalAssertions} assertions</div>`;

  if(f.issues.length){
    html+='<h4>Issues ('+f.issues.length+')</h4><div class="issue-list">';
    /* sort: error > warning > info */
    const sevOrd={error:0,warning:1,info:2};
    const sorted=[...f.issues].sort((a,b)=>sevOrd[a.severity]-sevOrd[b.severity]);
    for(const i of sorted){
      html+=`<div class="issue-item"><span class="sev sev-${i.severity}">${i.severity}</span><span class="msg">${esc(i.message)}</span><span class="rule-tag">L${i.line} ${esc(i.rule)}</span>`;
      if(i.suggestion) html+=`<span class="suggestion">→ ${esc(i.suggestion)}</span>`;
      html+='</div>';
    }
    html+='</div>';
  }else{
    html+='<div style="color:var(--muted);font-size:.8125rem">No issues found – nice work!</div>';
  }

  if(f.recommendations&&f.recommendations.length){
    html+='<h4 style="margin-top:1rem">Recommendations</h4><ul class="rec-list">';
    for(const r of f.recommendations) html+='<li>'+esc(r)+'</li>';
    html+='</ul>';
  }
  return html;
}

/* ── main render ── */
function render(){
  const files=getVisible();
  $('#count-badge').textContent=files.length+' of '+DATA.length+' files';

  const main=$('#main');
  if(!files.length){
    main.innerHTML='<div class="empty">No files match your filters.</div>';
    return;
  }

  const folders=groupByFolder(files);
  let html='';
  for(const[dir,items] of folders){
    const avg=Math.round(items.reduce((s,f)=>s+f.score,0)/items.length);
    const gc=gradeColor(items.length===1?items[0].grade:avg>=90?'A':avg>=80?'B':avg>=70?'C':avg>=60?'D':'F');
    const totalIssues=items.reduce((s,f)=>s+f.issues.length,0);
    const isOpen=openDetails.has('folder:'+dir);

    html+=`<div class="folder">`;
    html+=`<div class="folder-hdr${isOpen?' open':''}" data-folder="${esc(dir)}">`;
    html+=`<span class="chevron">▶</span>`;
    html+=`<span class="fname">${esc(dir)}</span>`;
    html+=`<span class="badge c-${gc}">${avg}%</span>`;
    html+=`<span style="font-size:.6875rem;color:var(--muted)">${items.length} file${items.length>1?'s':''}${totalIssues?' · '+totalIssues+' issue'+(totalIssues>1?'s':''):''}</span>`;
    html+=`</div>`;
    html+=`<div class="folder-body${isOpen?'':' collapsed'}">`;

    for(const f of items){
      const gc2=gradeColor(f.grade);
      const isDetailOpen=openDetails.has(f.path);
      html+=`<div class="file-row" data-path="${esc(f.path)}">`;
      html+=`<span class="fpath">${esc(f.fileName)}</span>`;
      html+=`<span class="mini-bar"><span style="width:${f.score}%;background:${barColor(f.score)}"></span></span>`;
      html+=`<span class="score-pill grade-${f.grade}">${f.score}</span>`;
      html+=`<span class="tests-col">${f.totalTests}</span>`;
      html+=`<span class="issues-col${f.issues.length?' c-red':''}">${f.issues.length||'–'}</span>`;
      html+=`</div>`;
      html+=`<div class="detail${isDetailOpen?' open':''}" id="det-${esc(f.path)}">${isDetailOpen?renderDetail(f):''}</div>`;
    }
    html+=`</div></div>`;
  }
  main.innerHTML=html;
  bindEvents();
}

/* ── event binding ── */
function bindEvents(){
  $$('.folder-hdr').forEach(el=>{
    el.onclick=()=>{
      const dir=el.dataset.folder;
      const key='folder:'+dir;
      if(openDetails.has(key)){openDetails.delete(key)}else{openDetails.add(key)}
      render();
    };
  });
  $$('.file-row').forEach(el=>{
    el.onclick=()=>{
      const p=el.dataset.path;
      if(openDetails.has(p)){openDetails.delete(p)}else{openDetails.add(p)}
      /* also ensure parent folder is open */
      const f=DATA.find(x=>x.path===p);
      if(f){
        const parts=f.path.replace(/\\/g,'/').split('/');parts.pop();
        openDetails.add('folder:'+parts.join('/'));
      }
      render();
    };
  });
}

/* ── controls ── */
$('#search').addEventListener('input',e=>{query=e.target.value;render()});
$$('#grade-filter .pill').forEach(btn=>{
  btn.onclick=()=>{
    $$('#grade-filter .pill').forEach(b=>b.classList.remove('active'));
    btn.classList.add('active');
    activeGrade=btn.dataset.grade;
    render();
  };
});
$('#sort-sel').addEventListener('change',e=>{sortBy=e.target.value;render()});

/* ── init ── */
renderStats();
renderSidebar();
/* Auto-open all folders if ≤ 5 folders */
const folderSet=new Set(DATA.map(f=>{const p=f.path.replace(/\\/g,'/').split('/');p.pop();return 'folder:'+(p.join('/')||'.')}));
if(folderSet.size<=5) folderSet.forEach(k=>openDetails.add(k));
render();

})();
</script>
"##
    }
}

impl Default for HtmlReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Score, ScoreBreakdown, TestFramework, TestStats, TestType};
    use std::path::PathBuf;

    fn make_result(path: &str, score: u8) -> AnalysisResult {
        AnalysisResult {
            file_path: PathBuf::from(path),
            score: Score::new(score),
            breakdown: ScoreBreakdown {
                assertion_quality: 20,
                error_coverage: 18,
                boundary_conditions: 15,
                test_isolation: 17,
                input_variety: 15,
                ai_smells: 25,
            },
            transparent_breakdown: None,
            test_scores: None,
            issues: vec![],
            stats: TestStats {
                total_tests: 3,
                ..TestStats::default()
            },
            framework: TestFramework::Jest,
            test_type: TestType::Unit,
            source_file: None,
        }
    }

    #[test]
    fn test_report_contains_structure() {
        let r1 = make_result("src/auth/login.test.ts", 72);
        let r2 = make_result("src/auth/signup.test.ts", 88);
        let r3 = make_result("src/utils/format.test.ts", 45);
        let stats = AggregateStats {
            files_analyzed: 3,
            average_score: Score::new(68),
            total_tests: 9,
            total_issues: 5,
        };
        let reporter = HtmlReporter::new();
        let html = reporter.report(&[r1, r2, r3], &stats);

        assert!(html.contains("Rigor"));
        assert!(html.contains("const DATA="));
        assert!(html.contains("login.test.ts"));
        assert!(html.contains("signup.test.ts"));
        assert!(html.contains("format.test.ts"));
        assert!(html.contains("files:3"));
        assert!(html.contains("avg:68"));
    }

    #[test]
    fn test_escape_json_for_script() {
        assert_eq!(
            escape_json_for_script("</script>alert(1)"),
            "<\\/script>alert(1)"
        );
        assert_eq!(escape_json_for_script("normal"), "normal");
    }

    #[test]
    fn test_report_with_issues() {
        let mut r = make_result("test.ts", 60);
        r.issues.push(crate::Issue {
            rule: crate::Rule::WeakAssertion,
            severity: Severity::Warning,
            message: "Use toBe() instead".into(),
            location: crate::Location::new(5, 1),
            suggestion: Some("Replace with toBe".into()),
            fix: None,
        });
        let stats = AggregateStats {
            files_analyzed: 1,
            average_score: Score::new(60),
            total_tests: 3,
            total_issues: 1,
        };
        let reporter = HtmlReporter::new();
        let html = reporter.report(&[r], &stats);
        assert!(html.contains("weak-assertion"));
        assert!(html.contains("Use toBe() instead"));
    }

    #[test]
    fn test_empty_results() {
        let stats = AggregateStats {
            files_analyzed: 0,
            average_score: Score::new(0),
            total_tests: 0,
            total_issues: 0,
        };
        let reporter = HtmlReporter::new();
        let html = reporter.report(&[], &stats);
        assert!(html.contains("const DATA=[]"));
    }
}
