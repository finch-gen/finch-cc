use std::fs::File;
use std::error::Error;
use std::io::prelude::*;
use std::process::Command;
use finch_frontend_api::{
  FinchClass,
  FinchNew,
  FinchDrop,
  FinchMethod,
  FinchStatic,
  FinchGetter,
  FinchSetter,
  TypeKind,
  get_package_name,
};

trait ToCPP {
  fn to_header(&self) -> String;
  fn to_impl(&self) -> String;
}

impl ToCPP for FinchNew {
  fn to_header(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].display_name, name));
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
      args.push(format!("{} {}", self.arg_types[i].display_name, name));
    }

    format!("
      {}::{0}({}) {{
        this->self = {}({});
      }}",
      self.class_name,
      args.join(", "),
      self.fn_name,
      self.arg_names.join(", "),
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
      args.push(format!("{} {}", self.arg_types[i].display_name, name));
    }

    format!("
      {}
      {} {}({});",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.ret_type.display_name,
      self.method_name,
      args.join(", ")
    )
  }

  fn to_impl(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].display_name, name));
    }

    let mut s = format!("
      {} {}::{}({}) {{
        assert((\"The internal pointer on this object is no longer valid. Either the destructor or a method that consumes the internal pointer has been called.\", this->self != nullptr));",
      self.ret_type.display_name,
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
          self.arg_names.iter().map(|x| format!(", {}", x)).collect::<String>(),
        ).as_str();
      } else {
        s += format!("
            auto value = {}(this->self{});
            this->self = nullptr;
            return value;
          }}",
          self.fn_name,
          self.arg_names.iter().map(|x| format!(", {}", x)).collect::<String>(),
        ).as_str();
      }
    } else {
      s += format!("
          return {}(this->self{});
        }}",
        self.fn_name,
        self.arg_names.iter().map(|x| format!(", {}", x)).collect::<String>(),
      ).as_str();
    }

    s
  }
}

impl ToCPP for FinchStatic {
  fn to_header(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].display_name, name));
    }

    format!("
      {}
      static {} {}({});",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.ret_type.display_name,
      self.method_name,
      args.join(", ")
    )
  }

  fn to_impl(&self) -> String {
    let mut args = Vec::new();
    for (i, name) in self.arg_names.iter().enumerate() {
      args.push(format!("{} {}", self.arg_types[i].display_name, name));
    }

    format!("
      {} {}::{}({}) {{
        return {}({});
      }}",
      self.ret_type.display_name,
      self.class_name,
      self.method_name,
      args.join(", "),
      self.fn_name,
      self.arg_names.join(", ")
    )
  }
}

impl ToCPP for FinchGetter {
  fn to_header(&self) -> String {
    format!("
      {}
      {} get_{}();",
      self.comments.as_ref().unwrap_or(&"".to_string()),
      self.type_.display_name,
      self.field_name)
  }

  fn to_impl(&self) -> String {
    format!("
      {} {}::get_{}() {{
        assert((\"The internal pointer on this object is no longer valid. Either the destructor or a method that consumes the internal pointer has been called.\", this->self != nullptr));
        return {}(this->self);
      }}",
      self.type_.display_name,
      self.class_name,
      self.field_name,
      self.fn_name,
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
      self.type_.display_name,
    )
  }

  fn to_impl(&self) -> String {
    format!("
      void {}::set_{}({} value) {{
        assert((\"The internal pointer on this object is no longer valid. Either the destructor or a method that consumes the internal pointer has been called.\", this->self != nullptr));
        return {}(this->self, value);
      }}",
      self.class_name,
      self.field_name,
      self.type_.display_name,
      self.fn_name,
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

pub fn generate() -> Result<(), Box<dyn Error>> {
  let name = get_package_name()?;
  let name_underscore = name.replace("-", "_");

  let output = finch_frontend_api::generate()?;

  let header_name = format!("{}.h", name_underscore);
  let mut header_file = File::create(&header_name)?;
  let impl_name = format!("{}-impl.h", name_underscore);
  let mut impl_file = File::create(&impl_name)?;

  header_file.write_fmt(format_args!("
    #pragma once;
    
    #include <cstdarg>
    #include <cstdint>
    #include <cstdlib>
    #include <cassert>
    #include <new>

    #include \"{}-finch_bindgen.h\"

    namespace {0} {{
      
      /// This function must be called before using this library
      /// to initialize the internals.
      void initialize();\n",
    name_underscore
  ))?;
  
  impl_file.write_fmt(format_args!(
    "#pragma once;
    
    #include <cstdarg>
    #include <cstdint>
    #include <cstdlib>
    #include <cassert>
    #include <new>

    namespace {0} {{
      
      void initialize() {{
        finch::bindgen::{0}::___finch_bindgen___{0}___initialize();
      }}\n",
    name_underscore
  ))?;

  for class in output.classes {
    header_file.write_fmt(format_args!("{}\n", class.1.to_header()))?;
    impl_file.write_fmt(format_args!("{}\n", class.1.to_impl()))?;
  }

  header_file.write_fmt(format_args!("\n}}\n\n#include \"{}-impl.h\"", name_underscore))?;
  impl_file.write(b"\n}")?;

  Command::new("clang-format")
    .arg("--style=Google")
    .arg("-i")
    .arg(format!("{}-finch_bindgen.h", name_underscore))
    .arg(header_name)
    .arg(impl_name)
    .status()?;

  Ok(())
}
