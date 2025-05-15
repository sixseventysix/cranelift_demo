use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module, Linkage};

#[derive(Clone, Debug)]
pub enum Node {
    X,
    Y,
    Number(f32),
    Add(Box<Node>, Box<Node>),
    Mul(Box<Node>, Box<Node>),
    Sin(Box<Node>),
    Cos(Box<Node>),
}

fn codegen_node(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    node: &Node,
    x: Value,
    y: Value,
) -> Value {
    match node {
        Node::X => x,
        Node::Y => y,
        Node::Number(val) => builder.ins().f32const(*val),
        Node::Add(a, b) => {
            let lhs = codegen_node(builder, module, a, x, y);
            let rhs = codegen_node(builder, module, b, x, y);
            builder.ins().fadd(lhs, rhs)
        }
        Node::Mul(a, b) => {
            let lhs = codegen_node(builder, module, a, x, y);
            let rhs = codegen_node(builder, module, b, x, y);
            builder.ins().fmul(lhs, rhs)
        }
        Node::Sin(a) => {
            let arg = codegen_node(builder, module, a, x, y);

            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(types::F32));
            sig.returns.push(AbiParam::new(types::F32));

            let sinf_func = module
                .declare_function("sinf", Linkage::Import, &sig)
                .unwrap();
            let local = module.declare_func_in_func(sinf_func, builder.func);
            let call_inst = builder.ins().call(local, &[arg]);
            builder.inst_results(call_inst)[0]
        }
        Node::Cos(a) => {
            let arg = codegen_node(builder, module, a, x, y);

            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(types::F32));
            sig.returns.push(AbiParam::new(types::F32));

            let cosf_func = module
                .declare_function("cosf", Linkage::Import, &sig)
                .unwrap();
            let local = module.declare_func_in_func(cosf_func, builder.func);
            let call_inst = builder.ins().call(local, &[arg]);
            builder.inst_results(call_inst)[0]
        }
    }
}

fn build_jit_function(ast: &Node) -> Box<dyn Fn(f32, f32) -> f32> {
    let builder = JITBuilder::new(cranelift_module::default_libcall_names())
        .expect("Failed to create JITBuilder");
    let mut module = JITModule::new(builder);

    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(types::F32));
    sig.params.push(AbiParam::new(types::F32));
    sig.returns.push(AbiParam::new(types::F32));

    let func_id = module
        .declare_function("jit_func", Linkage::Export, &sig)
        .unwrap();

    let mut ctx = module.make_context();
    ctx.func.signature = sig;

    let mut builder_ctx = FunctionBuilderContext::new();
    let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
    let block = fb.create_block();

    fb.append_block_params_for_function_params(block);
    fb.switch_to_block(block);
    fb.seal_block(block);

    let x = fb.block_params(block)[0];
    let y = fb.block_params(block)[1];
    let result = codegen_node(&mut fb, &mut module, ast, x, y);
    fb.ins().return_(&[result]);
    fb.finalize();

    println!("{}", ctx.func.display());
    module.define_function(func_id, &mut ctx).unwrap();
    module.clear_context(&mut ctx);
    let _ = module.finalize_definitions();

    let code = module.get_finalized_function(func_id);
    let fn_ptr = unsafe { std::mem::transmute::<_, fn(f32, f32) -> f32>(code) };
    Box::new(fn_ptr)
}

fn main() {
    let ast = Node::Add(Box::new(Node::X), Box::new(Node::Sin(Box::new(Node::Y))));
    let jit_fn = build_jit_function(&ast);
    println!("f(1.0, 0.5) = {}", jit_fn(1.0, 0.5));
}