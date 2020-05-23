use std::fs::File;
use std::sync::Mutex;
use std::error::Error;
use std::path::PathBuf;
use std::io::prelude::*;
use std::process::Command;
use std::collections::HashSet;
use lazy_static::lazy_static;
use finch_frontend_api::{
  FinchClass,
  FinchNew,
  FinchDrop,
  FinchMethod,
  FinchStatic,
  FinchGetter,
  FinchSetter,
  FinchType,
  TypeKind,
  get_package_name,
};

static mut CRATE_NAME: String = String::new();

static mut USE_OPTIONAL: bool = false;

lazy_static! {
  static ref TEMPLATES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

trait ToCPPType {
  fn to_cpp_type(&self) -> String;
  fn convert_arg(&self, body: String) -> String;
  fn convert_ret(&self, body: String) -> String;
}

impl ToCPPType for FinchType {
  fn to_cpp_type(&self) -> String {
    if let Some(canonical_type) = self.canonical_type.as_ref() {
      return canonical_type.to_cpp_type();
    }
  
    match self.kind {
      TypeKind::Void |
      TypeKind::Bool |
      TypeKind::CharS | TypeKind::CharU | TypeKind::SChar | TypeKind::UChar |
      TypeKind::Short | TypeKind::UShort | TypeKind::Int | TypeKind::UInt |
      TypeKind::Long | TypeKind::ULong | TypeKind::LongLong | TypeKind::ULongLong |
      TypeKind::Float | TypeKind::Double => self.display_name.clone(),
  
      TypeKind::Record => {
        if self.display_name == format!("finch::bindgen::{}::FinchString", unsafe { &CRATE_NAME }) {
          "std::string".to_string()
        } else if self.display_name.starts_with(format!("finch::bindgen::{}::FinchOption", unsafe { &CRATE_NAME }).as_str()) {
          unsafe { USE_OPTIONAL = true };
          format!("nonstd::optional<{}>", self.template_argument_types.as_ref().unwrap()[0].as_ref().unwrap().to_cpp_type())
        } else if self.display_name.starts_with(format!("finch::bindgen::{}::FinchResult", unsafe { &CRATE_NAME }).as_str()) {
          self.template_argument_types.as_ref().unwrap()[0].as_ref().unwrap().to_cpp_type()
        } else {
          panic!("unknown type {}", self.display_name)
        }
      },
  
      _ => panic!("unknown type {}", self.display_name)
    }
  }

  fn convert_arg(&self, body: String) -> String {
    if let Some(canonical_type) = self.canonical_type.as_ref() {
      return canonical_type.convert_arg(body);
    }
  
    match self.kind {
      TypeKind::Void |
      TypeKind::Bool |
      TypeKind::CharS | TypeKind::CharU | TypeKind::SChar | TypeKind::UChar |
      TypeKind::Short | TypeKind::UShort | TypeKind::Int | TypeKind::UInt |
      TypeKind::Long | TypeKind::ULong | TypeKind::LongLong | TypeKind::ULongLong |
      TypeKind::Float | TypeKind::Double => body,
  
      TypeKind::Record => {
        if self.display_name == format!("finch::bindgen::{}::FinchString", unsafe { &CRATE_NAME }) {
          format!("
            [](std::string str) -> finch::bindgen::{}::FinchString {{
              return finch::bindgen::{0}::___finch_bindgen___{0}___builtin___FinchString___new(reinterpret_cast<const uint8_t *>(str.data()), str.size());
            }}({})",
            unsafe { &CRATE_NAME },
            body
          )
        } else if self.display_name.starts_with(format!("finch::bindgen::{}::FinchOption", unsafe { &CRATE_NAME }).as_str()) {
          unsafe { USE_OPTIONAL = true };
          let original_inner_type = self.template_argument_types.as_ref().unwrap()[0].as_ref().unwrap();
          let inner_type = original_inner_type.to_cpp_type();
          let inner_body = original_inner_type.convert_arg("opt.value()".to_string());
  
          TEMPLATES.lock().unwrap().insert(format!("template struct FinchOption<{}>;", original_inner_type.display_name));
  
          format!("
            [](nonstd::optional<{inner_type}> opt) -> finch::bindgen::{crate_name}::FinchOption<{original_inner_type}> {{
              finch::bindgen::{crate_name}::FinchOption<{original_inner_type}> finch;
              if (opt.has_value()) {{
                finch.tag = finch::bindgen::{crate_name}::FinchOption<{original_inner_type}>::Tag::Some;
                finch.some = {{ {inner_body} }};
              }} else {{
                finch.tag = finch::bindgen::{crate_name}::FinchOption<{original_inner_type}>::Tag::None;
              }}
              return finch;
            }}({body})",
            crate_name=unsafe { &CRATE_NAME },
            original_inner_type=original_inner_type.display_name,
            inner_type=inner_type,
            inner_body=inner_body,
            body=body,
          )
        } else {
          panic!("unknown type {}", self.display_name)
        }
      },
  
      _ => panic!("unknown type {}", self.display_name)
    }
  }

  fn convert_ret(&self, body: String) -> String {
    if let Some(canonical_type) = self.canonical_type.as_ref() {
      return canonical_type.convert_ret(body);
    }
  
    match self.kind {
      TypeKind::Void |
      TypeKind::Bool |
      TypeKind::CharS | TypeKind::CharU | TypeKind::SChar | TypeKind::UChar |
      TypeKind::Short | TypeKind::UShort | TypeKind::Int | TypeKind::UInt |
      TypeKind::Long | TypeKind::ULong | TypeKind::LongLong | TypeKind::ULongLong |
      TypeKind::Float | TypeKind::Double => body,
  
      TypeKind::Record => {
        if self.display_name == format!("finch::bindgen::{}::FinchString", unsafe { &CRATE_NAME }) {
          format!("
            [](finch::bindgen::{}::FinchString finch) -> std::string {{
              std::string str(finch.ptr, finch.len);
              finch::bindgen::{0}::
                  ___finch_bindgen___{0}___builtin___FinchString___drop(finch);
              return str;
            }}({})",
            unsafe { &CRATE_NAME },
            body
          )
        } else if self.display_name.starts_with(format!("finch::bindgen::{}::FinchOption", unsafe { &CRATE_NAME }).as_str()) {
          unsafe { USE_OPTIONAL = true };
          let original_inner_type = self.template_argument_types.as_ref().unwrap()[0].as_ref().unwrap();
          let inner_type = original_inner_type.to_cpp_type();
          let inner_body = original_inner_type.convert_ret("finch.some._0".to_string());
  
          TEMPLATES.lock().unwrap().insert(format!("template struct FinchOption<{}>;", original_inner_type.display_name));
  
          format!("
            [](finch::bindgen::{crate_name}::FinchOption<{original_inner_type}> finch) -> nonstd::optional<{inner_type}> {{
              if (finch.tag == finch::bindgen::{crate_name}::FinchOption<{original_inner_type}>::Tag::Some) {{
                return nonstd::optional<{inner_type}>({inner_body});
              }} else {{
                return nonstd::nullopt;
              }}
            }}({body})",
            crate_name=unsafe { &CRATE_NAME },
            original_inner_type=original_inner_type.display_name,
            inner_type=inner_type,
            inner_body=inner_body,
            body=body,
          )
        } else if self.display_name.starts_with(format!("finch::bindgen::{}::FinchResult", unsafe { &CRATE_NAME }).as_str()) {
          let original_inner_type = self.template_argument_types.as_ref().unwrap()[0].as_ref().unwrap();
          let inner_type = original_inner_type.to_cpp_type();
          let inner_body = original_inner_type.convert_ret("finch.ok._0".to_string());
  
          TEMPLATES.lock().unwrap().insert(format!("template struct FinchResult<{}>;", original_inner_type.display_name));
  
          format!(r#"
            [](finch::bindgen::{crate_name}::FinchResult<{original_inner_type}> finch) -> {inner_type} {{
              if (finch.tag == finch::bindgen::{crate_name}::FinchResult<{original_inner_type}>::Tag::Ok) {{
                return {inner_body};
              }} else {{
                std::string str(finch.err._0.ptr, finch.err._0.len);
                finch::bindgen::{crate_name}::___finch_bindgen___{crate_name}___builtin___FinchString___drop(finch.err._0);
                #ifdef finch_bindgen_EXCEPTIONS
                  throw std::runtime_error(str);
                #else
                  std::cout << "fatal: Result returned Err(\"" << str << "\")" << std::endl;
                  abort();
                #endif
              }}
            }}({body})"#,
            crate_name=unsafe { &CRATE_NAME },
            original_inner_type=original_inner_type.display_name,
            inner_type=inner_type,
            inner_body=inner_body,
            body=body,
          )
        } else {
          panic!("unknown type {}", self.display_name)
        }
      },
  
      _ => panic!("unknown type {}", self.display_name)
    }
  }
}

trait ToCPP {
  fn to_header(&self) -> String;
  fn to_impl(&self) -> String;
}

impl ToCPP for FinchNew {
  fn to_header(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].to_cpp_type(), name));
    }

    format!("
      {}
      {}({});",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.class_name,
      args.join(", ")
    )
  }

  fn to_impl(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].to_cpp_type(), name));
    }

    format!("
      {}::{0}({}) {{
        this->self = {}({});
      }}",
      self.class_name,
      args.join(", "),
      self.fn_name,
      self.arg_names.iter().enumerate().map(|(i, x)| {
        self.arg_types[i].convert_arg(x.clone())
      }).collect::<Vec<String>>().join(", "),
    )
  }
}

impl ToCPP for FinchDrop {
  fn to_header(&self) -> String {
    format!("  ~{}();", self.class_name)
  }

  fn to_impl(&self) -> String {
    format!("
      {}::~{0}() {{
        if (this->self) {{
          {}(this->self);
        }}
      }}",
      self.class_name,
      self.fn_name,
    )
  }
}

impl ToCPP for FinchMethod {
  fn to_header(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].to_cpp_type(), name));
    }

    format!("
      {}
      {} {}({});",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.ret_type.to_cpp_type(),
      self.method_name,
      args.join(", ")
    )
  }

  fn to_impl(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].to_cpp_type(), name));
    }

    let mut s = format!("
      {} {}::{}({}) {{
        assert((\"The internal pointer on this object is no longer valid. Either the destructor or a method that consumes the internal pointer has been called.\", this->self != nullptr));",
      self.ret_type.to_cpp_type(),
      self.class_name,
      self.method_name,
      args.join(", "),
    );

    if self.consume {
      if self.ret_type.kind == TypeKind::Void {
        s += format!("
          {}(this->self{});
            this->self = nullptr;
          }}",
          self.fn_name,
          self.arg_names.iter().enumerate().map(|(i, x)| {
            self.arg_types[i].convert_arg(x.clone())
          }).collect::<Vec<String>>().join(", "),
        ).as_str();
      } else {
        let body = self.ret_type.convert_ret("value".to_string());
        s += format!("
            auto value = {}(this->self{});
            this->self = nullptr;
            return {};
          }}",
          self.fn_name,
          self.arg_names.iter().enumerate().map(|(i, x)| {
            self.arg_types[i].convert_arg(x.clone())
          }).collect::<Vec<String>>().join(", "),
          body
        ).as_str();
      }
    } else {
      let body = self.ret_type.convert_ret(format!("{}(this->self{})", self.fn_name, self.arg_names.iter().enumerate().map(|(i, x)| {
        self.arg_types[i].convert_arg(x.clone())
      }).collect::<Vec<String>>().join(", ")));

      s += format!("
          {};
        }}",
        body,
      ).as_str();
    }

    s
  }
}

impl ToCPP for FinchStatic {
  fn to_header(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].to_cpp_type(), name));
    }

    format!("
      {}
      static {} {}({});",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.ret_type.to_cpp_type(),
      self.method_name,
      args.join(", ")
    )
  }

  fn to_impl(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].to_cpp_type(), name));
    }

    let body = self.ret_type.convert_ret(format!("{}({})", self.fn_name, self.arg_names.iter().enumerate().map(|(i, x)| {
      self.arg_types[i].convert_arg(x.clone())
    }).collect::<Vec<String>>().join(", ")));

    format!("
      {} {}::{}({}) {{
        return {};
      }}",
      self.ret_type.to_cpp_type(),
      self.class_name,
      self.method_name,
      args.join(", "),
      body,
    )
  }
}

impl ToCPP for FinchGetter {
  fn to_header(&self) -> String {
    format!("
      {}
      {} get_{}();",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.type_.to_cpp_type(),
      self.field_name)
  }

  fn to_impl(&self) -> String {
    let body = self.type_.convert_ret(format!("{}(this->self)", self.fn_name));

    format!("
      {} {}::get_{}() {{
        assert((\"The internal pointer on this object is no longer valid. Either the destructor or a method that consumes the internal pointer has been called.\", this->self != nullptr));
        return {};
      }}",
      self.type_.to_cpp_type(),
      self.class_name,
      self.field_name,
      body,
    )
  }
}

impl ToCPP for FinchSetter {
  fn to_header(&self) -> String {
    format!("
      {}
      void set_{}({} value);",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.field_name,
      self.type_.to_cpp_type(),
    )
  }

  fn to_impl(&self) -> String {
    let body = self.type_.convert_arg("value".to_string());

    format!("
      void {}::set_{}({} value) {{
        assert((\"The internal pointer on this object is no longer valid. Either the destructor or a method that consumes the internal pointer has been called.\", this->self != nullptr));
        return {}(this->self, {});
      }}",
      self.class_name,
      self.field_name,
      self.type_.to_cpp_type(),
      self.fn_name,
      body,
    )
  }
}

impl ToCPP for FinchClass {
  fn to_header(&self) -> String {
    format!("
      {}
      class {} {{
      public:
      {}
      {}

      {}

      {}

      {}

      {}

      private:
        {1}(const {1}&) = delete;
        {1} &operator=(const {1}&) = delete;
        {} *self = nullptr;
      }};",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.name,
      self.new.as_ref().map_or("".to_string(), |x| x.to_header()),
      self.drop.as_ref().map_or("".to_string(), |x| x.to_header()),
      self.statics.iter().map(|x| x.to_header()).collect::<Vec<String>>().join("\n\n"),
      self.methods.iter().map(|x| x.to_header()).collect::<Vec<String>>().join("\n\n"),
      self.getters.iter().map(|x| x.to_header()).collect::<Vec<String>>().join("\n\n"),
      self.setters.iter().map(|x| x.to_header()).collect::<Vec<String>>().join("\n\n"),
      self.c_name,
    )
  }

  fn to_impl(&self) -> String {
    format!("
      {}
      {}
      {}
      {}
      {}
      {}",
      self.new.as_ref().map_or("".to_string(), |x| x.to_impl()),
      self.drop.as_ref().map_or("".to_string(), |x| x.to_impl()),
      self.statics.iter().map(|x| x.to_impl()).collect::<Vec<String>>().join("\n"),
      self.methods.iter().map(|x| x.to_impl()).collect::<Vec<String>>().join("\n"),
      self.getters.iter().map(|x| x.to_impl()).collect::<Vec<String>>().join("\n"),
      self.setters.iter().map(|x| x.to_impl()).collect::<Vec<String>>().join("\n"),
    )
  }
}

fn copy_third_party(config: &Config) -> Result<(), Box<dyn Error>> {
  if unsafe { USE_OPTIONAL } {
    let mut file = File::create(config.out_dir.join("include").join("optional.h"))?;

    file.write_all(include_bytes!("../third_party/optional.hpp"))?;
  }
  
  Ok(())
}

fn generate_cmake(config: &Config) -> Result<(), Box<dyn Error>> {
  if config.generate_cmake {
    let mut file = File::create(config.out_dir.join("CMakeLists.txt"))?;

    let abs_include_dir = config.out_dir.join("include");
    let include_dir = abs_include_dir.strip_prefix(std::env::current_dir().unwrap())?;

    file.write_fmt(format_args!("set(CRATE_NAME \"{}\")\n", get_package_name()?))?;
    file.write_fmt(format_args!("set({}_INCLUDE_DIR \"${{CMAKE_CURRENT_SOURCE_DIR}}/{}\")\n\n", get_package_name()?, include_dir.display()))?;
    file.write(include_bytes!("../CMakeLists.txt.in"))?;
  }
  
  Ok(())
}

#[derive(Clone, Debug)]
pub struct Config {
  out_dir: PathBuf,
  generate_cmake: bool,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      out_dir: std::env::current_dir().unwrap(),
      generate_cmake: true,
    }
  }
}

impl Config {
  fn to_frontend_cfg(&self) -> finch_frontend_api::Config {
    finch_frontend_api::Config {
      out_dir: Some(self.out_dir.join("include")),
    }
  }

  pub fn generate(self) -> Result<(), Box<dyn Error>> {
    let name = get_package_name()?;
    let name_underscore = name.replace("-", "_");

    unsafe {
      CRATE_NAME = name_underscore.clone();
    }
  
    let output = self.to_frontend_cfg().generate()?;
  
    let header_name = self.out_dir.join("include").join(format!("{}.h", name_underscore));
    let impl_name = self.out_dir.join("include").join(format!("{}-impl.h", name_underscore));

    let mut header_file = File::create(&header_name)?;
    let mut impl_file = File::create(&impl_name)?;
  
    let mut header_content = String::new();
    let mut impl_content = String::new();
    for class in output.classes {
      header_content += &format!("{}\n", class.1.to_header());
      impl_content += &format!("{}\n", class.1.to_impl());
    }

    let mut bindgen_file = File::open(self.out_dir.join("include").join(format!("{}-finch_bindgen.h", name_underscore)))?;
    let mut bindgen_content = String::new();
    bindgen_file.read_to_string(&mut bindgen_content)?;
    drop(bindgen_file);

    let bindgen_content = bindgen_content.replace(
      "extern \"C\" {",
      &(TEMPLATES.lock().unwrap().clone().into_iter().collect::<Vec<String>>().join("\n") + "\n\nextern \"C\" {"),
    );

    let mut bindgen_file = File::create(self.out_dir.join("include").join(format!("{}-finch_bindgen.h", name_underscore)))?;
    bindgen_file.write_all(bindgen_content.as_bytes())?;

    let mut includes = "
      #include <cstdarg>
      #include <cstdint>
      #include <cstdlib>
      #include <cassert>
      #include <new>\n".to_string();

    if unsafe { USE_OPTIONAL } {
      includes += "#include \"optional.h\"\n";
    }

    header_file.write_fmt(format_args!("
      #pragma once
      
      {}
  
      #include \"{}-finch_bindgen.h\"
  
      namespace {1} {{\n",
      includes,
      name_underscore
    ))?;
    
    impl_file.write_fmt(format_args!("
      #pragma once
      
      {}
  
      #if defined(__cpp_exceptions) || defined(__EXCEPTIONS) || defined(_CPPUNWIND)
        #define finch_bindgen_EXCEPTIONS
      #endif

      namespace {1} {{\n",
      includes,
      name_underscore
    ))?;

    header_file.write(header_content.as_bytes())?;
    impl_file.write(impl_content.as_bytes())?;

    header_file.write_fmt(format_args!("\n}}\n\n#include \"{}-impl.h\"", name_underscore))?;
    impl_file.write(b"\n}")?;
  
    copy_third_party(&self)?;
    generate_cmake(&self)?;

    Command::new("clang-format")
      .arg("--style=Google")
      .arg("-i")
      .arg(format!("{}-finch_bindgen.h", name_underscore))
      .arg(header_name)
      .arg(impl_name)
      .status()?;
  
    Ok(())
  }
}

#[derive(Clone, Debug, Default)]
pub struct Builder {
  config: Config,
}

impl Builder {
  pub fn new() -> Self {
    std::default::Default::default()
  }

  pub fn with_out_dir<T: Into<PathBuf>>(mut self, out_dir: T) -> Self {
    self.config.out_dir = out_dir.into();
    self
  }

  pub fn with_generate_cmake(mut self, value: bool) -> Self {
    self.config.generate_cmake = value;
    self
  }

  pub fn generate(self) -> Result<(), Box<dyn Error>> {
    self.config.generate()
  }
}
