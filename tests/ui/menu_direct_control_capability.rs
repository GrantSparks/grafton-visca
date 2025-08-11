// This test verifies that only cameras with HasDirectMenuControl trait
// can access direct menu control methods.

use grafton_visca::{
    camera::{Camera, profiles::{GenericVisca}},
};

fn main() {
    // Use a mock transport for testing
    struct MockTransport;
    
    impl grafton_visca::transport::Transport for MockTransport {
        type Error = std::io::Error;
        type SendFut<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Self::Error>> + Send + 'a>>;
        type RecvFut<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<bytes::Bytes, Self::Error>> + Send + 'a>>;
        
        fn send<'a>(&'a self, _data: &'a [u8]) -> Self::SendFut<'a> {
            Box::pin(async { Ok(()) })
        }
        
        fn recv(&self) -> Self::RecvFut<'_> {
            Box::pin(async { Ok(bytes::Bytes::new()) })
        }
    }
    
    let transport = MockTransport;
    
    // Create a camera without direct menu control support
    let generic_camera = Camera::<GenericVisca, _>::new(transport);
    
    // This should fail to compile - GenericVisca doesn't implement HasDirectMenuControl
    // We're not importing the trait, so these methods should not be found
    generic_camera.direct_menu_control(0x01, 0x02);
    //~^ ERROR: no method named `direct_menu_control` found
    
    generic_camera.toggle_menu();
    //~^ ERROR: no method named `toggle_menu` found
}