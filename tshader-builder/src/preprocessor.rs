use anyhow::bail;
use lazy_static::lazy_static;
use log::debug;
use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, tag, take_while},
    character::complete::{digit0, digit1, satisfy, space0, space1},
    combinator::{consumed, cut, map_opt, opt, peek, recognize, value},
    error::context,
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, terminated, tuple},
    IResult, InputTakeAtPosition,
};
use petgraph::stable_graph::NodeIndex;
use rand::{distributions::Alphanumeric, Rng};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::{Display, Write},
    path::PathBuf,
    rc::Rc,
    str::FromStr,
};
use strum::*;

#[derive(Debug, Default, Clone)]
pub struct PreprocessorConfig {
    includes: Vec<PathBuf>,
    defines: HashMap<String, String>,
}

impl PreprocessorConfig {
    pub fn with_include<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.includes.push(path.into());
        self
    }

    pub fn with_define<T: Into<String>, S: Into<String>>(mut self, key: T, value: S) -> Self {
        self.defines.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug)]
struct IncludeNode<'a> {
    path: String,
    commands: Option<Vec<Command<'a>>>,
    includes: Vec<NodeIndex<u32>>,
    content: String,
}

#[derive(Clone)]
enum EvalVal {
    Number(i64),
    Float(f64),
    String(String),
    ContextFn(Rc<dyn Fn(&[EvalVal], &mut HashMap<String, EvalVal>) -> anyhow::Result<EvalVal>>),
    Bool(bool),
    None,
}

impl From<&Literal> for EvalVal {
    fn from(value: &Literal) -> Self {
        match value {
            Literal::Float(f) => EvalVal::Float(*f),
            Literal::Number(n) => EvalVal::Number(*n),
            Literal::String(s) => EvalVal::String(s.to_string()),
        }
    }
}

impl From<bool> for EvalVal {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for EvalVal {
    fn from(value: i64) -> Self {
        Self::Number(value)
    }
}

impl From<f64> for EvalVal {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<()> for EvalVal {
    fn from(_: ()) -> Self {
        Self::None
    }
}

impl From<String> for EvalVal {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl Display for EvalVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalVal::Number(n) => n.fmt(f),
            EvalVal::Float(v) => v.fmt(f),
            EvalVal::String(str) => f
                .write_char('"')
                .and_then(|_| f.write_str(str))
                .and_then(|_| f.write_char('"')),
            EvalVal::None => f.write_str("None"),
            EvalVal::Bool(b) => b.fmt(f),
            EvalVal::ContextFn(_) => f.write_str("fn"),
        }
    }
}

type IncludeGraph<'a> = petgraph::graph::DiGraph<IncludeNode<'a>, (), u32>;

fn atomic_counter(
    inputs: &[EvalVal],
    ctx: &mut HashMap<String, EvalVal>,
) -> anyhow::Result<EvalVal> {
    let mut beg = 0;
    let mut step = 1;
    if !inputs.is_empty() {
        if let EvalVal::Number(i) = inputs[0] {
            beg = i;
        } else {
            anyhow::bail!("invalid parameter type");
        }
    }
    if inputs.len() == 2 {
        if let EvalVal::Number(i) = inputs[1] {
            step = i;
        } else {
            anyhow::bail!("invalid parameter type");
        }
    } else {
        anyhow::bail!("invalid parameter count");
    }
    let hidden_name = format!(
        "__HIDDEN__{}",
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect::<String>()
    );
    ctx.insert(hidden_name.clone(), EvalVal::Number(beg));
    Ok(EvalVal::ContextFn(Rc::new(move |_, ctx| {
        let val = ctx.get_mut(&hidden_name).unwrap();
        if let EvalVal::Number(n) = val {
            let val = *n;
            *n += step;
            return Ok(EvalVal::Number(val));
        }
        anyhow::bail!("atomic_counter invalid type");
    })))
}

pub struct ParsedLocation {
    pub group: u32,
    pub binding: u32,
}

pub struct Variable {
    pub location: ParsedLocation,
    pub option: Option<String>, // builtin?
}

struct IfCondition {
    disabled: bool,
    enter: bool,
    depth: u32,
}

#[derive(Default)]
struct PreprocessorContext<'a> {
    config: PreprocessorConfig,
    graph: IncludeGraph<'a>,
    map: HashMap<String, NodeIndex<u32>>,
    file_path: PathBuf,
    base_path: PathBuf,
    distinct_nodes: HashSet<NodeIndex<u32>>,
    buf: String,
    var_map: HashMap<String, EvalVal>,
    built_in_func: HashMap<
        String,
        Box<dyn Fn(&[EvalVal], &mut HashMap<String, EvalVal>) -> anyhow::Result<EvalVal>>,
    >,
    loc_struct_map: HashMap<String, HashMap<String, Variable>>,
    loc_global_map: HashMap<String, HashMap<String, Variable>>,

    loc_struct_state: HashMap<String, ParsedLocation>,

    loc_global_state: HashMap<String, ParsedLocation>,
    global_group_state: u32,
}

pub struct PreprocessorResult {
    pub data: String,
    pub loc_struct_map: HashMap<String, HashMap<String, Variable>>,
    pub loc_global_map: HashMap<String, HashMap<String, Variable>>,
}

impl<'a> PreprocessorContext<'a> {
    fn new(config: PreprocessorConfig, path: PathBuf) -> anyhow::Result<Self> {
        let mut var_map: HashMap<String, EvalVal> = HashMap::new();
        var_map.insert("True".to_owned(), true.into());
        var_map.insert("False".to_owned(), false.into());
        var_map.insert("None".to_owned(), ().into());
        for (key, value) in config.defines.iter() {
            if var_map.contains_key(value) {
                var_map.insert(key.to_owned(), var_map.get(value).unwrap().clone());
            } else {
                if value.is_empty() {
                    var_map.insert(key.to_owned(), ().into());
                    continue;
                }
                let (output, lit) = literal(unsafe { std::mem::transmute(value.as_str()) })?;
                if !output.is_empty() {
                    anyhow::bail!("parse define key {} fail", key);
                }
                var_map.insert(key.clone(), (&lit).into());
            }
        }

        let mut res = Self {
            config,
            file_path: path.clone(),
            base_path: path.parent().unwrap().to_path_buf(),
            var_map,
            ..Default::default()
        };
        res.built_in_func
            .insert("_atomic_counter".to_owned(), Box::new(atomic_counter));

        Ok(res)
    }

    fn detect_include_file_path(&self, source: &str) -> anyhow::Result<String> {
        for inc in std::iter::once(&self.base_path).chain(self.config.includes.iter()) {
            let target = inc.join(source);
            if target.exists() {
                return Ok(target
                    .canonicalize()
                    .map_err(|e| anyhow::anyhow!("{:?} {:?} in {:?}", e, source, target))?
                    .to_str()
                    .expect("to path string fail")
                    .to_owned());
            }
        }
        bail!("include file {} not found", source)
    }

    fn process_includes(&mut self, node: NodeIndex<u32>) -> anyhow::Result<Vec<NodeIndex<u32>>> {
        let mut ret = Vec::new();
        let mut include_nodes = vec![];
        let includes: Vec<_> = {
            let n = self.graph.node_weight(node).unwrap();
            n.commands
                .as_ref()
                .unwrap()
                .iter()
                .filter_map(|v| {
                    if let Command::Include(i) = v {
                        Some(i.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };
        for include in includes {
            let path_str = self.detect_include_file_path(include.source)?;
            if !self.map.contains_key(&path_str) {
                let idx = self.parse_file(path_str.clone())?;
                ret.push(idx);
            }
            let idx = self.map.get(&path_str).unwrap();
            self.graph.add_edge(node, *idx, ());
            include_nodes.push(*idx);
        }
        self.graph.node_weight_mut(node).unwrap().includes = include_nodes;

        Ok(ret)
    }

    fn build_common(&mut self, node: NodeIndex<u32>) -> anyhow::Result<()> {
        let mut to_build = VecDeque::new();
        to_build.push_back(node);

        // parse all includes
        while let Some(node) = to_build.pop_front() {
            let res = self.process_includes(node)?;
            for iter in res {
                to_build.push_back(iter);
            }
        }

        if petgraph::algo::is_cyclic_directed(&self.graph) {
            let gz =
                petgraph::dot::Dot::with_config(&self.graph, &[petgraph::dot::Config::EdgeNoLabel]);
            return Err(anyhow::format_err!("cyclic detected in includes {:?}", gz));
        }

        Ok(())
    }

    fn eval(&mut self, expr: &Expr) -> anyhow::Result<EvalVal> {
        match expr {
            Expr::Binary(b) => {
                let l = self.eval(&b.left)?;
                let r = self.eval(&b.right)?;
                b.op.eval(&l, &r)
            }
            Expr::Unary(u) => {
                let l = self.eval(&u.right)?;
                u.op.eval(&l)
            }
            Expr::Ident(i) => Ok(self.var_map.get(*i).cloned().unwrap_or(EvalVal::None)),
            Expr::Call(c) => {
                let ident_name = match c.ident.as_ref() {
                    Expr::Ident(ident) => *ident,
                    _ => return Err(anyhow::anyhow!("fail parse function call expr")),
                };

                let mut vals = Vec::new();
                for expr in &c.exprs {
                    vals.push(self.eval(expr)?);
                }
                match self.built_in_func.get(ident_name) {
                    Some(func) => func(&vals, &mut self.var_map),
                    None => Ok(EvalVal::None),
                }
            }
            Expr::Literal(l) => Ok(l.into()),
        }
    }

    fn build_commands(&mut self, node: NodeIndex<u32>) -> anyhow::Result<()> {
        let r = self.graph.node_weight_mut(node).unwrap();
        let commands = r.commands.take().unwrap();
        debug!("{:?}", commands);

        let mut index = 0;
        let mut if_stack = Vec::<IfCondition>::new();

        for command in commands {
            match command {
                Command::Include(_) => {
                    let include_node = self.graph.node_weight(node).unwrap().includes[index];
                    index += 1;
                    if !self.distinct_nodes.contains(&include_node) {
                        self.distinct_nodes.insert(include_node);

                        self.build_commands(include_node)?;
                        self.buf.write_str("\n")?;
                    }
                }
                Command::If(cond) => {
                    let mut enter = match self.eval(&cond.cond)? {
                        EvalVal::Bool(ok) => ok,
                        EvalVal::None => false,
                        _ => return Err(anyhow::anyhow!("if expr result type is not expected")),
                    };
                    let mut disabled = enter;

                    if let Some(v) = if_stack.last() {
                        if !v.enter {
                            enter = false;
                            disabled = true;
                        }
                    }

                    log::trace!("enter [{}] if {:?}", enter, cond.cond);
                    if_stack.push(IfCondition {
                        enter,
                        disabled,
                        depth: 1,
                    });
                }
                Command::ElseIf(cond) => {
                    let enter = match self.eval(&cond.cond)? {
                        EvalVal::Bool(ok) => ok,
                        EvalVal::None => false,
                        _ => return Err(anyhow::anyhow!("if expr result type is not expected")),
                    };
                    if let Some(v) = if_stack.last() {
                        log::trace!(
                            "enter [{}] before {} elseif {:?}",
                            enter,
                            v.disabled,
                            cond.cond
                        );
                        if !v.disabled {
                            if_stack.push(IfCondition {
                                disabled: enter,
                                enter,
                                depth: v.depth + 1,
                            });
                        } else {
                            // do not enter
                            if_stack.push(IfCondition {
                                disabled: true,
                                enter: false,
                                depth: v.depth + 1,
                            });
                        }
                    } else {
                        return Err(anyhow::anyhow!("elseif condition is not expected"));
                    }
                }
                Command::Else(_cond) => {
                    if let Some(v) = if_stack.last() {
                        log::trace!("enter [true] before {} else", v.disabled);
                        if !v.disabled {
                            if_stack.push(IfCondition {
                                disabled: true,
                                enter: true,
                                depth: v.depth + 1,
                            });
                        } else {
                            // donot enter
                            if_stack.push(IfCondition {
                                disabled: true,
                                enter: false,
                                depth: v.depth + 1,
                            });
                        }
                    } else {
                        return Err(anyhow::anyhow!("else condition is not expected"));
                    }
                }
                Command::EndIf(_cond) => {
                    if if_stack.len() == 0 {
                        return Err(anyhow::anyhow!("endif condition is not expected"));
                    }
                    let last_condition = if_stack.pop().unwrap();
                    log::trace!("pop {} endif", last_condition.depth - 1);
                    for _ in 0..(last_condition.depth - 1) {
                        if_stack.pop();
                    }
                }
                Command::Raw(raw) => {
                    if let Some(v) = if_stack.last() {
                        if !v.enter {
                            continue;
                        }
                    }
                    self.buf.write_str(raw)?;
                }
                Command::Decl(decl) => {
                    if let Some(v) = if_stack.last() {
                        if !v.enter {
                            continue;
                        }
                    }
                    if let Some(expr) = decl.expr {
                        let val = self.eval(&expr)?;
                        self.var_map.insert(decl.ident.to_owned(), val);
                    } else {
                        self.var_map.insert(decl.ident.to_owned(), true.into());
                    }
                }
                Command::Assign(assign) => {
                    if let Some(v) = if_stack.last() {
                        if !v.enter {
                            continue;
                        }
                    }
                    if let Some(expr) = assign.expr {
                        let val = self.eval(&expr)?;
                        self.var_map.insert(assign.ident.to_owned(), val);
                    } else {
                        self.var_map.insert(assign.ident.to_owned(), true.into());
                    }
                }
                Command::Reference(ident) => {
                    if let Some(v) = if_stack.last() {
                        if !v.enter {
                            continue;
                        }
                    }
                    let val = self.var_map.get(ident.target).cloned();
                    if let Some(val) = val {
                        if let EvalVal::ContextFn(f) = val {
                            let val = f(&[], &mut self.var_map)?;
                            self.buf.write_str(&val.to_string())?;
                        } else {
                            self.buf.write_str(&val.to_string())?;
                        }
                    } else {
                    }
                }
                Command::LocStruct(loc) => {
                    if let Some(v) = if_stack.last() {
                        if !v.enter {
                            continue;
                        }
                    }
                    if !self.loc_struct_state.contains_key(loc.name) {
                        self.loc_struct_state.insert(
                            loc.name.to_string(),
                            ParsedLocation {
                                group: 0,
                                binding: 0,
                            },
                        );
                    }

                    let option = loc.builtin.map(|v| v.to_string());
                    let state = self.loc_struct_state.get_mut(loc.name).unwrap();
                    let mut cur_binding = state.binding;

                    if let Some(option) = &option {
                        self.buf
                            .write_fmt(format_args!("@builtin({}) {}", option, loc.var_name))?;
                        cur_binding = 0;
                    } else {
                        self.buf.write_fmt(format_args!(
                            "@location({}) {}",
                            cur_binding, loc.var_name
                        ))?;
                        state.binding += 1;
                    }

                    self.loc_struct_map
                        .entry(loc.name.to_string())
                        .or_default()
                        .insert(
                            loc.var_name.to_string(),
                            Variable {
                                location: ParsedLocation {
                                    binding: cur_binding,
                                    group: 0,
                                },
                                option,
                            },
                        );
                }
                Command::LocGlobal(loc) => {
                    if let Some(v) = if_stack.last() {
                        if !v.enter {
                            continue;
                        }
                    }
                    if !self.loc_global_state.contains_key(loc.name) {
                        self.loc_global_state.insert(
                            loc.name.to_string(),
                            ParsedLocation {
                                group: self.global_group_state,
                                binding: 0,
                            },
                        );
                        self.global_group_state += 1;
                    }

                    let state = self.loc_global_state.get_mut(loc.name).unwrap();
                    let cur_binding = state.binding;
                    let cur_group = state.group;

                    let option = loc.var_tag.map(|v| v.to_string());

                    if let Some(option) = &option {
                        let push_constant =  option == "push_constant";
                        if push_constant {
                            self.buf.write_fmt(format_args!(
                                "var<{}> {}",
                                option, loc.var_name
                            ))?;
                        } else {
                            self.buf.write_fmt(format_args!(
                                "@group({}) @binding({}) var<{}> {}",
                                cur_group, cur_binding, option, loc.var_name
                            ))?;
                            state.binding += 1;
                        }
                    } else {
                        self.buf.write_fmt(format_args!(
                            "@group({}) @binding({}) var {}",
                            cur_group, cur_binding, loc.var_name
                        ))?;
                        state.binding += 1;
                    }

                    self.loc_global_map
                        .entry(loc.name.to_string())
                        .or_default()
                        .insert(
                            loc.var_name.to_string(),
                            Variable {
                                location: ParsedLocation {
                                    binding: cur_binding,
                                    group: cur_group,
                                },
                                option,
                            },
                        );
                }
            }
        }

        Ok(())
    }

    fn build(mut self) -> anyhow::Result<PreprocessorResult> {
        let path = self
            .file_path
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("{:?} {:?}", e, self.file_path))?;
        let path_str = path.to_str().expect("path to string fail").to_owned();
        let node = self.parse_file(path_str)?;
        self.build_common(node)?;

        self.build_commands(node)?;

        Ok(
            PreprocessorResult {
                data: self.buf,
                loc_global_map: self.loc_global_map,
                loc_struct_map: self.loc_struct_map,
            }
        )
    }

    fn parse_file(&mut self, path: String) -> anyhow::Result<NodeIndex<u32>> {
        let content =
            std::fs::read_to_string(&path).map_err(|e| anyhow::anyhow!("{:?} {:?}", e, &path))?;
        self.parse_file_content(path, content)
    }

    fn parse_file_content(
        &mut self,
        path: String,
        content: String,
    ) -> anyhow::Result<NodeIndex<u32>> {
        let mut node = IncludeNode {
            path: path.clone(),
            commands: None,
            includes: vec![],
            content,
        };
        let content = unsafe { std::mem::transmute(node.content.as_str()) };
        let (_, commands) =
            parse_commands(content).map_err(|e| anyhow::anyhow!("parse {:?} {:?}", path, e))?;
        node.commands = Some(commands);
        let idx = self.graph.add_node(node);
        self.map.insert(path, idx);
        Ok(idx)
    }
}

pub struct Preprocessor {
    config: PreprocessorConfig,
}

impl Preprocessor {
    pub fn new(config: PreprocessorConfig) -> Self {
        Self { config }
    }

    pub fn process<P: Into<PathBuf>>(&self, path: P) -> anyhow::Result<PreprocessorResult> {
        let ctx: PreprocessorContext<'_> =
            PreprocessorContext::new(self.config.clone(), path.into())?;
        ctx.build()
    }
}

#[derive(Debug)]
struct Decl<'a> {
    ident: &'a str,
    expr: Option<Box<Expr<'a>>>,
}

#[derive(Debug)]
struct Assign<'a> {
    ident: &'a str,
    expr: Option<Box<Expr<'a>>>,
}

#[derive(Debug)]
struct If<'a> {
    cond: Box<Expr<'a>>,
}

#[derive(Debug)]
struct ElseIf<'a> {
    cond: Box<Expr<'a>>,
}

#[derive(Debug)]
struct Else {}

#[derive(Debug)]
struct EndIf {}

#[derive(Debug)]
struct LocGlobal<'a> {
    name: &'a str,
    var_name: &'a str,
    var_tag: Option<&'a str>,
}

#[derive(Debug)]
struct LocStruct<'a> {
    name: &'a str,
    var_name: &'a str,
    builtin: Option<&'a str>,
}

#[derive(Debug)]
enum Expr<'a> {
    Binary(BinaryExpr<'a>),
    Unary(UnaryExpr<'a>),
    Ident(&'a str),
    Literal(Literal),
    Call(CallExpr<'a>),
}

#[derive(Debug)]
struct BinaryExpr<'a> {
    left: Box<Expr<'a>>,
    right: Box<Expr<'a>>,
    op: BinOp,
}

#[derive(Debug)]
struct UnaryExpr<'a> {
    right: Box<Expr<'a>>,
    op: UnaryOp,
}

#[derive(Debug)]
struct CallExpr<'a> {
    ident: Box<Expr<'a>>,
    exprs: Vec<Box<Expr<'a>>>,
}

#[derive(Debug, Clone)]
struct Include<'a> {
    source: &'a str,
}

#[derive(Debug)]
struct Reference<'a> {
    target: &'a str,
}

#[derive(Debug)]
enum Command<'a> {
    Include(Include<'a>),
    If(If<'a>),
    ElseIf(ElseIf<'a>),
    Else(Else),
    EndIf(EndIf),
    Raw(&'a str),
    Decl(Decl<'a>),
    Assign(Assign<'a>),
    Reference(Reference<'a>),
    LocStruct(LocStruct<'a>),
    LocGlobal(LocGlobal<'a>),
}

fn parse_commands(i: &str) -> IResult<&str, Vec<Command>> {
    many0(parse_command)(i)
}

fn raw(i: &str) -> IResult<&str, &str> {
    if i.is_empty() {
        return Err(nom::Err::Error(nom::error::make_error(
            i,
            nom::error::ErrorKind::Alpha,
        )));
    }
    let mut chars = i.chars();
    let mut len = 0;
    while let Some(ch) = chars.next() {
        if ch == '/' {
            if peek(tag::<_, _, nom::error::Error<_>>("//#"))(chars.as_str()).is_ok() {
                break;
            }
        } else if ch == '#' {
            break;
        } else if ch == '@' && len != 0 {
            break;
        }
        len += ch.len_utf8();
    }
    let (beg, end) = i.split_at(len);
    Ok((end, beg))
}
pub fn multispace_ex0(input: &str) -> IResult<&str, &str> {
    let mut chars = input.chars();
    let mut has_next = false;
    let mut n = 0;
    while let Some(ch) = chars.next() {
        if ch == ' ' || ch == '\t' {
            if has_next {
                break;
            }
        } else if ch == '\r' || ch == '\n' {
            has_next = true;
        } else {
            break;
        }
        n += ch.len_utf8();
    }
    let (beg, end) = input.split_at(n);
    Ok((end, beg))
}

fn parse_command(i: &str) -> IResult<&str, Command> {
    alt((
        map_opt(reference_cmd, |v| Some(Command::Reference(v))),
        preceded(
            tag("///#"),
            cut(alt((
                map_opt(include_cmd, |v| Some(Command::Include(v))),
                terminated(
                    alt((
                        map_opt(if_cmd, |v| Some(Command::If(v))),
                        map_opt(elseif_cmd, |v| Some(Command::ElseIf(v))),
                        map_opt(else_cmd, |v| Some(Command::Else(v))),
                        map_opt(endif_cmd, |v| Some(Command::EndIf(v))),
                        map_opt(decl_cmd, |v| Some(Command::Decl(v))),
                        map_opt(assign_cmd, |v| Some(Command::Assign(v))),
                    )),
                    opt(multispace_ex0),
                ),
            ))),
        ),
        map_opt(loc_global, |cmd| Some(Command::LocGlobal(cmd))),
        map_opt(loc_struct, |cmd| Some(Command::LocStruct(cmd))),
        map_opt(raw, |raw| Some(Command::Raw(raw))),
    ))(i)
}

fn include_cmd(i: &str) -> IResult<&str, Include> {
    preceded(
        tuple((tag("include"), space1)),
        map_opt(
            delimited(
                tag("\""),
                take_while(|c: char| c != '"' && !c.is_ascii_control() && c != '\n'),
                tag("\""),
            ),
            |v| Some(Include { source: v }),
        ),
    )(i)
}

#[derive(Debug, Clone, Copy, Display, EnumIter, EnumString, PartialEq, Eq, Hash)]
#[repr(u8)]
#[strum(serialize_all = "snake_case", use_phf)]
enum BinOp {
    #[strum(serialize = ">")]
    Greater,
    #[strum(serialize = ">=")]
    GreaterEqual,
    #[strum(serialize = "<")]
    Less,
    #[strum(serialize = "<=")]
    LessEqual,
    #[strum(serialize = "==")]
    Equal,
    #[strum(serialize = "||")]
    Or,
    #[strum(serialize = "&&")]
    And,
    #[strum(serialize = "+")]
    Add,
    #[strum(serialize = "-")]
    Subtract,
    #[strum(serialize = "*")]
    Multiply,
    #[strum(serialize = "/")]
    Divide,
    #[strum(serialize = "<<")]
    LeftShift,
    #[strum(serialize = ">>")]
    RightShift,

    #[strum(serialize = "(")]
    LeftParentheses,
    #[strum(serialize = ")")]
    RightParentheses,
}

impl BinOp {
    fn eval(&self, left: &EvalVal, right: &EvalVal) -> anyhow::Result<EvalVal> {
        Ok(match left {
            EvalVal::Number(l) => {
                if let EvalVal::Number(r) = right {
                    match self {
                        BinOp::Greater => (l > r).into(),
                        BinOp::GreaterEqual => (l >= r).into(),
                        BinOp::Less => (l < r).into(),
                        BinOp::LessEqual => (l <= r).into(),
                        BinOp::Equal => (l == r).into(),
                        BinOp::Add => (l + r).into(),
                        BinOp::Subtract => (l - r).into(),
                        BinOp::Multiply => (l * r).into(),
                        BinOp::Divide => (l / r).into(),
                        BinOp::LeftShift => (l << r).into(),
                        BinOp::RightShift => (l >> r).into(),
                        _ => return Err(op_is_not_support()),
                    }
                } else {
                    if let EvalVal::None = right {
                        (*l).into()
                    } else {
                        return Err(type_mismatch());
                    }
                }
            }
            EvalVal::Float(l) => {
                if let EvalVal::Float(r) = right {
                    match self {
                        BinOp::Greater => (l > r).into(),
                        BinOp::GreaterEqual => (l >= r).into(),
                        BinOp::Less => (l < r).into(),
                        BinOp::LessEqual => (l <= r).into(),
                        BinOp::Equal => (l == r).into(),
                        BinOp::Add => (l + r).into(),
                        BinOp::Subtract => (l - r).into(),
                        BinOp::Multiply => (l * r).into(),
                        BinOp::Divide => (l / r).into(),
                        _ => return Err(op_is_not_support()),
                    }
                } else {
                    if let EvalVal::None = right {
                        (*l).into()
                    } else {
                        return Err(type_mismatch());
                    }
                }
            }
            EvalVal::Bool(l) => {
                if let EvalVal::Bool(r) = right {
                    match self {
                        BinOp::Equal => (*l == *r).into(),
                        BinOp::Or => (*l || *r).into(),
                        BinOp::And => (*l && *r).into(),
                        _ => return Err(op_is_not_support()),
                    }
                } else {
                    if let EvalVal::None = right {
                        match self {
                            BinOp::Equal => false.into(),
                            BinOp::Or => (*l).into(),
                            BinOp::And => false.into(),
                            _ => return Err(op_is_not_support()),
                        }
                    } else {
                        return Err(type_mismatch());
                    }
                }
            }
            EvalVal::None => {
                if let EvalVal::None = right {
                    match self {
                        BinOp::Equal => ().into(),
                        BinOp::Or => ().into(),
                        BinOp::And => ().into(),
                        _ => return Err(op_is_not_support()),
                    }
                } else {
                    right.clone()
                }
            }
            _ => {
                return Err(type_mismatch());
            }
        })
    }
}

#[derive(Debug, Clone, Copy, Display, EnumIter, EnumString, PartialEq, Eq, Hash)]
#[repr(u8)]
#[strum(serialize_all = "snake_case", use_phf)]
enum UnaryOp {
    #[strum(serialize = "!")]
    Not,
    #[strum(serialize = "-")]
    Subtract,
}

fn type_mismatch() -> anyhow::Error {
    anyhow::anyhow!("type mismatch")
}

fn op_is_not_support() -> anyhow::Error {
    anyhow::anyhow!("op is not support")
}

impl UnaryOp {
    fn eval(&self, right: &EvalVal) -> anyhow::Result<EvalVal> {
        match self {
            UnaryOp::Subtract => match right {
                EvalVal::Number(n) => Ok({ -n }.into()),
                EvalVal::Float(n) => Ok({ -n }.into()),
                EvalVal::None => Ok(EvalVal::None),
                _ => Err(type_mismatch()),
            },
            UnaryOp::Not => match right {
                EvalVal::Bool(b) => Ok({ !b }.into()),
                EvalVal::None => Ok(EvalVal::None),
                _ => Err(type_mismatch()),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Infix {
    pub left_precedence: i32,
    pub right_precedence: i32,
}

impl Infix {
    pub const fn new_left_asso(p: i32) -> Self {
        Self {
            left_precedence: p,
            right_precedence: p + 1,
        }
    }
    pub const fn new_right_asso(p: i32) -> Self {
        Self {
            left_precedence: p + 1,
            right_precedence: p,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Prefix {
    pub precedence: i32,
}
impl Prefix {
    pub const fn new(p: i32) -> Self {
        Self { precedence: p }
    }
}

lazy_static! {
    static ref INFIX_MAP: HashMap<BinOp, Infix> = {
        let mut map = HashMap::new();
        map.insert(BinOp::Add, Infix::new_left_asso(100));
        map.insert(BinOp::Subtract, Infix::new_left_asso(100));
        map.insert(BinOp::Multiply, Infix::new_left_asso(150));
        map.insert(BinOp::Divide, Infix::new_left_asso(150));
        map.insert(BinOp::LeftParentheses, Infix::new_left_asso(10000));
        map.insert(BinOp::Greater, Infix::new_left_asso(10));
        map.insert(BinOp::GreaterEqual, Infix::new_left_asso(10));
        map.insert(BinOp::Less, Infix::new_left_asso(10));
        map.insert(BinOp::LessEqual, Infix::new_left_asso(10));
        map.insert(BinOp::Equal, Infix::new_left_asso(10));
        map.insert(BinOp::And, Infix::new_left_asso(90));
        map.insert(BinOp::Or, Infix::new_left_asso(80));
        map.insert(BinOp::LeftShift, Infix::new_right_asso(70));
        map.insert(BinOp::RightShift, Infix::new_right_asso(70));
        map
    };
}
lazy_static! {
    static ref PREFIX_MAP: HashMap<UnaryOp, Prefix> = {
        let mut map = HashMap::new();
        map.insert(UnaryOp::Subtract, Prefix::new(1000));
        map.insert(UnaryOp::Not, Prefix::new(1000));
        map
    };
}

#[derive(Debug)]
enum Literal {
    Float(f64),
    Number(i64),
    String(String),
}

impl<'a> PartialEq for Literal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Float(l0), Self::Float(r0)) => l0 == r0,
            (Self::Number(l0), Self::Number(r0)) => l0 == r0,
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Eq for Literal {}

#[derive(Debug, Eq, PartialEq)]
enum IdentOrOper<'a> {
    Ident(&'a str),
    BinOp(BinOp),
    UnaryOp(UnaryOp),
    Literal(Literal),
    Eof,
}

fn function_call(i: &str) -> IResult<&str, Vec<Box<Expr>>> {
    map_opt(
        delimited(
            tuple((space0, tag("("))),
            separated_list0(tuple((space0, tag(","))), expr),
            tuple((space0, tag(")"))),
        ),
        Some,
    )(i)
}

fn ident_or_oper(i: &str) -> IResult<&str, IdentOrOper> {
    let (input, _) = space0::<_, nom::error::Error<&str>>(i).unwrap();
    if input.is_empty() {
        return Ok((input, IdentOrOper::Eof));
    }
    let res = identifier(input);

    if let Ok((input, ident)) = res {
        return Ok((input, IdentOrOper::Ident(ident)));
    }
    let res = literal(input);
    if let Ok((input, literal)) = res {
        return Ok((input, IdentOrOper::Literal(literal)));
    }

    let mut i = 2;
    while i > 0 {
        if input.len() < i {
            break;
        }
        if let Ok(b) = BinOp::from_str(&input[..i]) {
            return Ok((&input[i..], IdentOrOper::BinOp(b)));
        }
        if let Ok(b) = UnaryOp::from_str(&input[..i]) {
            return Ok((&input[i..], IdentOrOper::UnaryOp(b)));
        }
        i -= 1;
    }
    Ok((input, IdentOrOper::Eof))
}

fn is_ident(lhs: &Expr) -> bool {
    match lhs {
        Expr::Ident(_) => true,
        _ => false,
    }
}

fn expr_pratt_parser(mut i: &str, precedence: i32) -> IResult<&str, Box<Expr>> {
    let (input, prefix) = preceded(space0, ident_or_oper)(i)?;
    i = input;
    let mut lhs = match prefix {
        IdentOrOper::Ident(ident) => Box::new(Expr::Ident(ident)),
        IdentOrOper::UnaryOp(oper) => {
            let (input, expr) = expr_pratt_parser(i, PREFIX_MAP.get(&oper).unwrap().precedence)?;
            i = input;
            Box::new(Expr::Unary(UnaryExpr {
                right: expr,
                op: oper,
            }))
        }
        IdentOrOper::BinOp(oper) => {
            if oper == BinOp::LeftParentheses {
                let (input, expr) = expr_pratt_parser(i, precedence)?;
                i = input;
                let (input, res) = preceded(space0, ident_or_oper)(i)?;
                if res != IdentOrOper::BinOp(BinOp::RightParentheses) {
                    return Err(nom::Err::Error(nom::error::make_error(
                        ")",
                        nom::error::ErrorKind::Alpha,
                    )));
                } else {
                    i = input;
                    expr
                }
            } else {
                return Err(nom::Err::Error(nom::error::make_error(
                    input,
                    nom::error::ErrorKind::Alpha,
                )));
            }
        }
        IdentOrOper::Eof => {
            return Err(nom::Err::Error(nom::error::make_error(
                input,
                nom::error::ErrorKind::Alpha,
            )));
        }
        IdentOrOper::Literal(literal) => Box::new(Expr::Literal(literal)),
    };
    loop {
        // peek
        let (input, infix) = preceded(space0, ident_or_oper)(i)?;
        match infix {
            IdentOrOper::Eof => break,
            IdentOrOper::BinOp(oper) => {
                if oper == BinOp::LeftParentheses && is_ident(&lhs) {
                    // function call
                    let (input, res) = function_call(i)?;
                    i = input;
                    lhs = Box::new(Expr::Call(CallExpr {
                        ident: lhs,
                        exprs: res,
                    }));
                } else if oper == BinOp::RightParentheses {
                    // eof
                    break;
                } else {
                    let infix_val = *INFIX_MAP.get(&oper).unwrap();
                    if infix_val.left_precedence < precedence {
                        break;
                    }
                    i = input;
                    let (input, expr) = expr_pratt_parser(i, infix_val.right_precedence)?;
                    i = input;
                    lhs = Box::new(Expr::Binary(BinaryExpr {
                        left: lhs,
                        right: expr,
                        op: oper,
                    }))
                }
            }
            _ => {
                return Err(nom::Err::Error(nom::error::make_error(
                    input,
                    nom::error::ErrorKind::Alpha,
                )));
            }
        }
    }
    Ok((i, lhs))
}

fn expr(i: &str) -> IResult<&str, Box<Expr>> {
    expr_pratt_parser(i, 0)
}

fn identifier2(i: &str) -> IResult<&str, &str> {
    recognize(tuple((
        satisfy(unicode_ident::is_xid_start),
        take_while(unicode_ident::is_xid_continue),
    )))(i)
}

fn identifier(i: &str) -> IResult<&str, &str> {
    context(
        "identifier",
        recognize(tuple((
            opt(nom::character::complete::char('_')),
            identifier2,
        ))),
    )(i)
}

fn str_normal(i: &str) -> IResult<&str, &str> {
    i.split_at_position1_complete(
        |item| item == '\"' || item == '\r',
        nom::error::ErrorKind::Digit,
    )
}

fn literal(i: &str) -> IResult<&str, Literal> {
    context(
        "literal",
        alt((
            map_opt(
                delimited(
                    tag("\""),
                    escaped_transform(
                        str_normal,
                        '\\',
                        alt((
                            value("\\", tag("\\")),
                            value("\"", tag("\"")),
                            value("\n", tag("n")),
                        )),
                    ),
                    tag("\""),
                ),
                |v| Some(Literal::String(v)),
            ),
            map_opt(
                consumed(tuple((digit1, opt(tuple((tag("."), digit0)))))),
                |(_, (z, k))| {
                    let z_int = i64::from_str(z).ok()?;

                    if let Some((_, s)) = k {
                        let s_int = u64::from_str(s).ok()?;
                        let mut f = z_int as f64;
                        f += s_int as f64 / s_int.ilog10() as f64;

                        Some(Literal::Float(f))
                    } else {
                        Some(Literal::Number(z_int))
                    }
                },
            ),
        )),
    )(i)
}

fn assign_inner(i: &str) -> IResult<&str, Option<Box<Expr>>> {
    opt(map_opt(
        preceded(
            // = expr
            tag("="),
            preceded(space0, expr),
        ),
        |e| Some(e),
    ))(i)
}

fn decl_cmd(i: &str) -> IResult<&str, Decl> {
    preceded(
        tuple((tag("decl"), space1)),
        map_opt(
            cut(tuple((identifier, space0, assign_inner))),
            |(ident, _, expr)| Some(Decl { ident, expr }),
        ),
    )(i)
}

fn assign_cmd(i: &str) -> IResult<&str, Assign> {
    preceded(
        tuple((tag("assign"), space1)),
        map_opt(
            cut(tuple((identifier, space0, assign_inner))),
            |(ident, _, expr)| Some(Assign { ident, expr }),
        ),
    )(i)
}

fn reference_cmd(i: &str) -> IResult<&str, Reference> {
    map_opt(
        preceded(
            tag("#"),
            delimited(
                tuple((tag("{"), space0)),
                identifier,
                tuple((space0, tag("}"))),
            ),
        ),
        |v| Some(Reference { target: v }),
    )(i)
}

fn if_cmd(i: &str) -> IResult<&str, If> {
    preceded(
        tuple((tag("if"), space1)),
        map_opt(expr, |e| Some(If { cond: e })),
    )(i)
}

fn elseif_cmd(i: &str) -> IResult<&str, ElseIf> {
    preceded(
        tuple((tag("elseif"), space1)),
        map_opt(expr, |e| Some(ElseIf { cond: e })),
    )(i)
}

fn endif_cmd(i: &str) -> IResult<&str, EndIf> {
    map_opt(tuple((tag("endif"),)), |_| Some(EndIf {}))(i)
}

fn else_cmd(i: &str) -> IResult<&str, Else> {
    map_opt(tuple((tag("else"),)), |_| Some(Else {}))(i)
}

fn loc_struct(i: &str) -> IResult<&str, LocStruct> {
    map_opt(
        tuple((
            tag("@loc_struct"),
            delimited(
                tuple((tag("("), space0)),
                identifier,
                tuple((space0, tag(")"))),
            ),
            space0,
            opt(map_opt(
                tuple((
                    tag("@builtin"),
                    delimited(
                        tuple((tag("("), space0)),
                        identifier,
                        tuple((space0, tag(")"))),
                    ),
                    space0,
                )),
                |(_, f, _)| Some(f),
            )),
            identifier,
        )),
        |(_, name, _, builtin, var_name)| {
            Some(LocStruct {
                name,
                var_name,
                builtin,
            })
        },
    )(i)
}

fn loc_global(i: &str) -> IResult<&str, LocGlobal> {
    map_opt(
        tuple((
            tag("@loc_global"),
            delimited(
                tuple((tag("("), space0)),
                identifier,
                tuple((space0, tag(")"))),
            ),
            space0,
            map_opt(
                tuple((
                    tag("var"),
                    opt(delimited(
                        tuple((space0, tag("<"), space0)),
                        identifier,
                        tuple((space0, tag(">"))),
                    )),
                )),
                |(_, is)| Some(is),
            ),
            space1,
            identifier,
        )),
        |(_, name, _, var_tag, _, var_name)| {
            Some(LocGlobal {
                name,
                var_name,
                var_tag,
            })
        },
    )(i)
}
